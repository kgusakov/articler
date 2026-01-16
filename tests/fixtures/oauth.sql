-- Insert test users for OAuth tests
INSERT INTO users (id, username, email, name, password_hash, created_at, updated_at) VALUES
    -- User for successful password grant test
    -- Password: "test_password_123"
    -- Hash generated with Argon2id
    (99, 'oauth_test_user', 'oauth@test.com', 'OAuth Test User', '$argon2id$v=19$m=19456,t=2,p=1$iQa9d2zJwn5CM1kfTd2fmg$i6rkIFF8e1D0hBbDJcrFdKUeRsDoyTGEvn1z7L66/60', 1687895144, 1687895850),

    -- User for invalid credentials test
    -- Password: "correct_password"
    (98, 'test_user_invalid', 'test@invalid.com', 'Test User Invalid', '$argon2id$v=19$m=19456,t=2,p=1$6jG+/nbMwdmvhLwJFE64pQ$+wcZ8ysDTTwgkVvryKcp4Q1gW/68Uf28vm9H916aYbU', 1687895144, 1687895850),

    -- User for invalid client test
    -- Password: "test_password"
    (97, 'test_user_client', 'test@client.com', 'Test User Client', '$argon2id$v=19$m=19456,t=2,p=1$WfwIWgdRFNkzMSZo3MoDVA$fqQU3CaLM8euh/ZhCMsVVaqBDZh0nUbDHpRGFesc2a0', 1687895144, 1687895850),

    -- User for refresh token test
    -- Password: "test_password"
    (96, 'refresh_test_user', 'refresh@test.com', 'Refresh Test User', '$argon2id$v=19$m=19456,t=2,p=1$WfwIWgdRFNkzMSZo3MoDVA$fqQU3CaLM8euh/ZhCMsVVaqBDZh0nUbDHpRGFesc2a0', 1687895144, 1687895850),

    -- User for invalid refresh token test
    (95, 'invalid_refresh_user', 'invalid@test.com', 'Invalid Refresh User', 'dummy_hash', 1687895144, 1687895850);

-- Insert OAuth clients
INSERT INTO clients (id, user_id, client_id, client_secret, created_at) VALUES
    -- Client for user 99 (oauth_test_user)
    (99, 99, 'test_client_id', 'test_client_secret', 1687895200),

    -- Client for user 98 (test_user_invalid)
    (98, 98, 'test_client', 'test_secret', 1687895200),

    -- Client for user 97 (test_user_client)
    (97, 97, 'valid_client', 'valid_secret', 1687895200),

    -- Client for user 96 (refresh_test_user)
    (96, 96, 'refresh_client', 'refresh_secret', 1687895200),

    -- Client for user 95 (invalid_refresh_user)
    (95, 95, 'invalid_refresh_client', 'invalid_refresh_secret', 1687895200);
