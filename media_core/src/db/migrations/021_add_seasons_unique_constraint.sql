-- Step 1: Update episodes to point to the canonical season ID (min id for the same show_id and season_number)
UPDATE episodes
SET season_id = (
    SELECT MIN(s2.id)
    FROM seasons s2
    JOIN seasons s1 ON s1.id = episodes.season_id
    WHERE s2.show_id = s1.show_id AND s2.season_number = s1.season_number
)
WHERE season_id IN (
    SELECT id FROM seasons
);

-- Step 2: Delete duplicate seasons
DELETE FROM seasons
WHERE id NOT IN (
    SELECT MIN(id)
    FROM seasons
    GROUP BY show_id, season_number
);

-- Step 3: Create the unique index to prevent future duplicates and satisfy the ON CONFLICT clause
CREATE UNIQUE INDEX IF NOT EXISTS idx_seasons_show_id_season_number ON seasons (show_id, season_number);
