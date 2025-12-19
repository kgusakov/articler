INSERT INTO entries (
    id,
    url,
    hashed_url,
    given_url,
    hashed_given_url,
    title,
    content,
    is_archived,
    archived_at,
    is_starred,
    starred_at,
    created_at,
    updated_at,
    mimetype,
    language,
    reading_time,
    domain_name,
    preview_picture,
    origin_url,
    published_at,
    published_by,
    is_public,
    uid
) VALUES (
    1,
    'https://a.com/1',
    'hash1',
    'https://a.com/g1',
    'ghash1',
    'title1',
    'content1',
    0,
    NULL,
    1,
    1702220400,
    1701428400,
    1702220400,
    'text/html',
    'en',
    8,
    'a.com',
    'https://a.com/pic1.jpg',
    'https://a.com/o1',
    1701424800,
    'author1',
    0,
    NULL
);

INSERT INTO tags (id, label, slug) VALUES
    (1, 'label1', 'slug1'),
    (2, 'label2', 'slug2'),
    (3, 'label3', 'slug3'),
    (4, 'label4', 'slug4');

INSERT INTO entry_tags (entry_id, tag_id) VALUES
    (1, 1),
    (1, 2),
    (1, 3),
    (1, 4);

INSERT INTO annotations (
    id,
    entry_id,
    annotator_schema_version,
    text,
    created_at,
    updated_at,
    quote
) VALUES
    (1, 1, 'v1', 'Note about async', 1701787200, 1701787200, 'Async is key'),
    (2, 1, 'v1', 'Example of sqlx', 1701855300, 1701855300, 'Sqlx checks queries');

INSERT INTO annotation_ranges (
    annotation_id,
    start,
    end,
    start_offset,
    end_offset
) VALUES
    (1, '/d[1]/p[3]', '/d[1]/p[3]', 0, 52),
    (1, '/d[1]/p[4]', '/d[1]/p[4]', 0, 28);

INSERT INTO annotation_ranges (
    annotation_id,
    start,
    end,
    start_offset,
    end_offset
) VALUES
    (2, '/d[2]/pre[1]', '/d[2]/pre[1]', 0, 65);