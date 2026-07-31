-- Insert test users for OAuth tests
INSERT INTO users (id, username, email, name, password_hash, created_at, updated_at) VALUES
    -- User for successful password grant test
    -- Password: "test_password_123"
    -- Hash generated with Argon2id
    (99, 'oauth_test_user', 'oauth@test.com', 'OAuth Test User', '$argon2id$v=19$m=8,t=1,p=1$1Xohw/wkkIT9Q7CJI32gXw$anfjRBgwsooqR7TjVnw4yBkcoCZLGNByv0wklD0xgWY', 1687895144, 1687895850),

    -- User for invalid credentials test
    -- Password: "correct_password"
    (98, 'test_user_invalid', 'test@invalid.com', 'Test User Invalid', '$argon2id$v=19$m=8,t=1,p=1$ouXuC/cqTcgGn4P8Nd4pUg$txXdf4od2EYxefbcC+y8S1XcTVdNSur2bIFyD/Woidg', 1687895144, 1687895850),

    -- User for invalid client test
    -- Password: "test_password"
    (97, 'test_user_client', 'test@client.com', 'Test User Client', '$argon2id$v=19$m=8,t=1,p=1$L/RTPSHncE4kgC6n9RV4RA$QEWlQ3wPY0xWdjdZ63DFjW/358Eb8uDXZIWVzyD/R8w', 1687895144, 1687895850),

    -- User for refresh token test
    -- Password: "test_password"
    (96, 'refresh_test_user', 'refresh@test.com', 'Refresh Test User', '$argon2id$v=19$m=8,t=1,p=1$L/RTPSHncE4kgC6n9RV4RA$QEWlQ3wPY0xWdjdZ63DFjW/358Eb8uDXZIWVzyD/R8w', 1687895144, 1687895850),

    -- User for invalid refresh token test
    (95, 'invalid_refresh_user', 'invalid@test.com', 'Invalid Refresh User', 'dummy_hash', 1687895144, 1687895850);

-- Insert OAuth clients
INSERT INTO clients (id, user_id, client_id, client_secret, name, created_at) VALUES
    -- Client for user 99 (oauth_test_user)
    (99, 99, 'test_client_id', 'test_client_secret', 'Test client 99', 1687895200),

    -- Client for user 98 (test_user_invalid)
    (98, 98, 'test_client', 'test_secret', 'Test client 98', 1687895200),

    -- Client for user 97 (test_user_client)
    (97, 97, 'valid_client', 'valid_secret', 'Test client 97', 1687895200),

    -- Client for user 96 (refresh_test_user)
    (96, 96, 'refresh_client', 'refresh_secret', 'Test client 96', 1687895200),

    -- Client for user 95 (invalid_refresh_user)
    (95, 95, 'invalid_refresh_client', 'invalid_refresh_secret', 'Test client 95', 1687895200);
