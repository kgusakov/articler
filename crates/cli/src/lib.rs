pub mod error;

use std::env;

use article_scraper::Scraper;
use db::repository::clients;
use db::repository::clients::ClientRow;
use db::repository::entries;
use db::repository::entries::FindParams;
use db::repository::users;
use email_address::EmailAddress;
use helpers::{generate_client_id, generate_client_secret, hash_password};
use sqlx::Pool;
use sqlx::Sqlite;
use url::Url;

use crate::error::{EmailInvalidSnafu, Result, UserNotFoundSnafu, UsernameBusySnafu};

pub async fn create_user(
    pool: &Pool<Sqlite>,
    username: &str,
    password: &str,
    name: &str,
    email: &str,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    if !EmailAddress::is_valid(email) {
        return EmailInvalidSnafu.fail();
    }

    if users::find_by_username(&mut *tx, username).await?.is_some() {
        return UsernameBusySnafu.fail();
    }

    let now = chrono::Utc::now().timestamp();
    users::create_user(
        &mut *tx,
        username,
        &hash_password(password)?,
        name,
        email,
        now,
        now,
    )
    .await?;

    tx.commit().await?;

    Ok(())
}

pub async fn create_client(
    pool: &Pool<Sqlite>,
    username: &str,
    client_name: &str,
) -> Result<ClientRow> {
    let mut tx = pool.begin().await?;

    if let Some(user) = users::find_by_username(&mut *tx, username).await? {
        let now = chrono::Utc::now().timestamp();
        let client = clients::create(
            &mut *tx,
            user.id,
            client_name,
            &generate_client_id(),
            &generate_client_secret(),
            now,
        )
        .await?;

        tx.commit().await?;

        Ok(client)
    } else {
        UserNotFoundSnafu.fail()
    }
}

pub async fn reload_articles(pool: &Pool<Sqlite>, username: &str) -> Result<()> {
    let mut tx = pool.begin().await?;

    let Some(user) = users::find_by_username(&mut tx, username).await? else {
        return UserNotFoundSnafu.fail();
    };

    let params = FindParams {
        user_id: user.id,
        ..Default::default()
    };

    let entries = entries::find_all(&mut tx, &params).await?;

    let proxy_scheme = match env::var("ALL_PROXY") {
        Ok(p) if !p.is_empty() => Some(p),
        _ => None,
    };

    let scraper = Scraper::new(proxy_scheme.as_deref())?;

    let mut i = 0;

    for e in entries.iter() {
        let e = &e.0;
        i += 1;

        println!("Reloading article {} {}/{}", e.id, i, entries.len());

        let url = Url::parse(&e.url)?;

        match scraper.extract(&url).await {
            Ok(doc) => {
                let preview_picture = doc.image_url.map(|u| u.to_string());

                let published_at = doc.published_at.map(|v| v.timestamp());

                let update = entries::UpdateEntry {
                    title: Some(Some(doc.title)),
                    content: Some(Some(doc.content_html)),
                    content_text: Some(Some(doc.content_text)),
                    reading_time: Some(Some(doc.reading_time)),
                    preview_picture: Some(preview_picture),
                    published_at: Some(published_at),
                    ..Default::default()
                };

                entries::update_by_id(&mut tx, user.id, e.id, update).await?;
            }
            Err(e) => println!("Content for {url} couldn't be parse or fetched: {e:?}"),
        }
    }

    tx.commit().await?;

    Ok(())
}
