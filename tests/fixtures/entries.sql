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
    'https://example.com/article/rust-web-backend/url',
    'a3f5e8d9c2b1a0f4e7d6c5b4a3f2e1d0',
    'https://example.com/article/rust-web-backend/given_url',
    'a3f5e8d9c2b1a0f4e7d6c5b4a3f2e1d0',
    'Building Web Backends with Rust',
    'This comprehensive guide covers building modern web backends using Rust, actix-web, and sqlx. Learn about async programming, database integration, and best practices for production-ready applications.',
    0,
    NULL,
    1,
    1702220400,  -- 2024-12-10 15:30:00 UTC
    1701428400,  -- 2024-12-01 10:00:00 UTC
    1702220400,  -- 2024-12-10 15:30:00 UTC
    'text/html',
    'en',
    8,
    'example.com',
    'https://example.com/images/rust-backend-preview.jpg',
    'https://example.com/article/rust-web-backend/origin',
    1701424800,  -- 2024-12-01 09:00:00 UTC
    'John Doe',
    0,
    NULL
);

-- Insert tags
INSERT INTO tags (id, label, slug) VALUES
    (1, 'Rust', 'rust'),
    (2, 'Web Development', 'web-development'),
    (3, 'Backend', 'backend'),
    (4, 'Tutorial', 'tutorial');

-- Link tags to the entry (many-to-many relationship)
INSERT INTO entry_tags (entry_id, tag_id) VALUES
    (1, 1),  -- Rust
    (1, 2),  -- Web Development
    (1, 3),  -- Backend
    (1, 4);  -- Tutorial

-- Insert annotations for the entry
INSERT INTO annotations (
    id,
    entry_id,
    annotator_schema_version,
    text,
    created_at,
    updated_at,
    quote
) VALUES
    (1,
     1,
     'v1.0',
     'Important section about async/await patterns in Rust',
     1701787200,  -- 2024-12-05 14:20:00 UTC
     1701787200,  -- 2024-12-05 14:20:00 UTC
     'Async programming is a cornerstone of modern web backends'),
    
    (2,
     1,
     'v1.0',
     'Great example of sqlx usage with compile-time checked queries',
     1701855300,  -- 2024-12-06 09:15:00 UTC
     1701855300,  -- 2024-12-06 09:15:00 UTC
     'The sqlx library provides compile-time verification of SQL queries');

-- Insert ranges for the first annotation
INSERT INTO annotation_ranges (
    annotation_id,
    start,
    end,
    start_offset,
    end_offset
) VALUES
    (1, '/div[1]/p[3]', '/div[1]/p[3]', 0, 52),
    (1, '/div[1]/p[4]', '/div[1]/p[4]', 0, 28);

-- Insert ranges for the second annotation
INSERT INTO annotation_ranges (
    annotation_id,
    start,
    end,
    start_offset,
    end_offset
) VALUES
    (2, '/div[2]/pre[1]', '/div[2]/pre[1]', 0, 65);

