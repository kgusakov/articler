INSERT INTO entries (
    id, user_id, url, hashed_url, given_url, hashed_given_url, title, content, content_text,
    is_archived, is_starred, created_at, updated_at, reading_time, domain_name
) VALUES
    (100, 1, 'https://x.com/u1',  'xh1', 'https://x.com/u1',  'xgh1', 'user1 entry',    '<p>c</p>', 'c', 0, 0, 1700000000, 1700000000, 1, 'x.com'),
    (200, 2, 'https://x.com/u2',  'xh2', 'https://x.com/u2',  'xgh2', 'user2 entry',    '<p>c</p>', 'c', 0, 0, 1700000000, 1700000000, 1, 'x.com'),
    (201, 2, 'https://x.com/u2b', 'xh3', 'https://x.com/u2b', 'xgh3', 'user2 untagged', '<p>c</p>', 'c', 0, 0, 1700000000, 1700000000, 1, 'x.com');
