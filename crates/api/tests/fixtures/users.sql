INSERT INTO users (id, username, email, name, password_hash, created_at, updated_at) VALUES
    (1, 'wallabag', 'wallabag@wallabag.io', 'Walla Baggger', '$argon2id$v=19$m=19456,t=2,p=1$hsWWj4oOAFTK2vLl7YjG0w$L+KcI0YL/8L8s2ZRRA9caoqEiyYE48Drm36y1KFk2bg', 1687895144, 1687895850),
    (2, 'test_user_2', 'test2@example.com', 'Test User 2', '$argon2id$v=19$m=19456,t=2,p=1$hsWWj4oOAFTK2vLl7YjG0w$L+KcI0YL/8L8s2ZRRA9caoqEiyYE48Drm36y1KFk2bg', 1687895144, 1687895850);

INSERT INTO clients (id, user_id, client_id, client_secret, name, created_at) VALUES
    (1, 1, 'client_1', 'secret_1', 'Client 1', 1687895200),
    (2, 1, 'client_2', 'secret_2', 'Client 2', 1687895300),
    (3, 1, 'android_client_id', 'android_client_secret', 'Android app', 1687895400),
    (4, 2, 'client_4', 'secret_4', 'Client 4', 1687895200);