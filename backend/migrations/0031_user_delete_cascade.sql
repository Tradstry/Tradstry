-- tags, tag_categories and usage_counters carry a user_id with no foreign key, so
-- deleting a users row left them behind pointing at a user that no longer exists.
DELETE FROM tags WHERE user_id NOT IN (SELECT id FROM users);
DELETE FROM tag_categories WHERE user_id NOT IN (SELECT id FROM users);
DELETE FROM usage_counters WHERE user_id NOT IN (SELECT id FROM users);

ALTER TABLE tags
    ADD CONSTRAINT tags_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE tag_categories
    ADD CONSTRAINT tag_categories_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE usage_counters
    ADD CONSTRAINT usage_counters_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;
