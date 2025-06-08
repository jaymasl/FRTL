CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TYPE user_rank AS ENUM ('Novice', 'Apprentice', 'Adept', 'Expert', 'Master');
CREATE TYPE essence_type AS ENUM ('Celestial', 'Ancient', 'Psychic', 'Undead', 'Fairy', 'Dark', 'Electric', 'Fire', 'Toxic', 'Construct', 'Air', 'Earth', 'Plant', 'Water', 'Fungal');
CREATE TYPE color_type AS ENUM ('Rainbow', 'Gold', 'Silver', 'Black', 'White', 'Purple', 'Green', 'Pink', 'Brown', 'Orange', 'Red', 'Blue');
CREATE TYPE art_style_type AS ENUM ('Wooden', 'Watercolor', 'Impressionism', 'Surrealism', 'Glass', 'Baroque', 'Gothic', 'Cubism', 'Abstract', 'Animated', 'Minimalist', 
    'Folk', 'Pixel', 'Graffiti', 'Anime', 'Pop', 'Sketch', 'Crayon', 'Doodle', 'Lowpoly', 'Papercraft', 'Plastic', 'Knit', 'Ceramic', 'Illusion', 'Retro', 'Plush', 'Metallic');
CREATE TYPE rarity_type AS ENUM ('Common', 'Uncommon', 'Rare', 'Epic', 'Legendary', 'Mythical');
CREATE TYPE market_status_type AS ENUM ('active', 'completed', 'cancelled');
CREATE TYPE market_type AS ENUM ('sale', 'auction');
CREATE TYPE event_type AS ENUM ('summoned', 'hatched', 'listed_for_sale', 'sale_cancelled', 'sold', 'transferred', 'traded', 'bid_placed', 'auction_won');
CREATE TYPE item_status AS ENUM ('available', 'locked', 'trading');
CREATE TYPE animal_type AS ENUM (
    'Dragon', 'Chimera', 'Cow', 'Unicorn', 'Lizard', 'Kraken', 'Megalodon',
    'Penguin', 'Mammoth', 'Tyrannosaurus', 'Pangolin', 'Bee', 'Whale',
    'Squid', 'Axolotl', 'Chameleon', 'Jellyfish', 'Mantis', 'Scorpion', 'Peacock', 'Parrot',
    'Eagle', 'Owl', 'Crow', 'Duck', 'Chicken', 'Crocodile', 'Turtle', 'Tiger',
    'Wolf', 'Lion', 'Jaguar', 'Fox', 'Dog', 'Cat', 'Rhinoceros', 'Bear', 'Deer', 'Dolphin',
    'Elephant', 'Crab', 'Raccoon', 'Sheep', 'Goat', 'Pig', 'Mouse', 'Hamster', 'Rabbit',
    'Squirrel', 'Rat', 'Frog', 'Horse', 'Donkey', 'Turkey', 'Goose', 'Llama',
    'Bison', 'Giraffe', 'Zebra', 'Panda', 'Kangaroo', 'Koala', 'Flamingo',
    'Spider', 'Sloth', 'Toucan', 'Otter', 'Alien'
);
CREATE TYPE order_side_type AS ENUM ('buy', 'sell');

CREATE OR REPLACE FUNCTION trigger_set_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    username VARCHAR(50) UNIQUE NOT NULL CHECK (length(username) >= 3),
    email VARCHAR(100) UNIQUE NOT NULL CHECK (email ~* '^[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}$'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMPTZ,
    currency_balance INTEGER NOT NULL DEFAULT 0 CHECK (currency_balance >= 0),
    last_login TIMESTAMPTZ,
    last_daily_reward TIMESTAMPTZ,
    claim_streak INTEGER NOT NULL DEFAULT 0,
    experience INTEGER NOT NULL DEFAULT 0 CHECK (experience >= 0),
    rank user_rank NOT NULL DEFAULT 'Novice',
    is_member BOOLEAN NOT NULL DEFAULT false,
    member_until TIMESTAMPTZ,
    membership_source VARCHAR(20) DEFAULT NULL
);

CREATE TABLE account_deletion_tokens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token VARCHAR(255) NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE scrolls (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    display_name VARCHAR(100) NOT NULL DEFAULT 'Summoning Scroll',
    image_path VARCHAR(255),
    description TEXT,
    quantity INTEGER NOT NULL DEFAULT 1 CHECK (quantity > 0),
    item_type VARCHAR(20) NOT NULL DEFAULT 'scroll',
    CONSTRAINT valid_item_type CHECK (item_type = 'scroll')
);

CREATE TABLE eggs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    summoned_by UUID NOT NULL REFERENCES users(id),
    incubation_ends_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP + INTERVAL '1 minute',
    essence essence_type NOT NULL,
    color color_type NOT NULL,
    art_style art_style_type NOT NULL,
    image_path VARCHAR(255),
    item_type VARCHAR(20) NOT NULL DEFAULT 'egg',
    display_name VARCHAR(100) NOT NULL DEFAULT 'Magical Egg',
    status item_status NOT NULL DEFAULT 'available',
    prompt TEXT,
    CONSTRAINT valid_item_type CHECK (item_type = 'egg')
);

CREATE TABLE creatures (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    original_egg_id UUID REFERENCES eggs(id) ON DELETE SET NULL,
    original_egg_summoned_by UUID REFERENCES users(id),
    hatched_by UUID NOT NULL REFERENCES users(id),
    original_egg_created_at TIMESTAMPTZ NOT NULL,
    essence essence_type NOT NULL,
    color color_type NOT NULL,
    art_style art_style_type NOT NULL,
    animal animal_type NOT NULL,
    rarity rarity_type NOT NULL,
    streak INTEGER NOT NULL DEFAULT 0 CHECK (streak >= 0),
    soul INTEGER NOT NULL DEFAULT 0 CHECK (soul >= 0),
    energy_full BOOLEAN NOT NULL DEFAULT false,
    energy_recharge_complete_at TIMESTAMPTZ,
    stats JSONB NOT NULL DEFAULT '{"health": 1, "attack": 1, "speed": 1}',
    hatched_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    image_path VARCHAR(255) NOT NULL,
    original_egg_image_path VARCHAR(255) NOT NULL,
    item_type VARCHAR(20) NOT NULL DEFAULT 'creature',
    display_name VARCHAR(100) NOT NULL,
    status item_status NOT NULL DEFAULT 'available',
    prompt TEXT,
    in_chaos_realm BOOLEAN NOT NULL DEFAULT false,
    chaos_realm_entry_at TIMESTAMPTZ,
    chaos_realm_reward_claimed BOOLEAN NOT NULL DEFAULT false,
    CONSTRAINT valid_item_type CHECK (item_type = 'creature')
);

CREATE TABLE item_references (
    id UUID PRIMARY KEY,
    item_type VARCHAR(20) NOT NULL,
    CONSTRAINT valid_item_type CHECK (item_type IN ('egg', 'creature', 'scroll'))
);

CREATE OR REPLACE FUNCTION maintain_item_references()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO item_references (id, item_type)
    VALUES (NEW.id, TG_ARGV[0]::VARCHAR)
    ON CONFLICT (id) DO NOTHING;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION cleanup_item_references()
RETURNS TRIGGER AS $$
BEGIN
    DELETE FROM item_references WHERE id = OLD.id;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER scroll_reference
    AFTER INSERT ON scrolls
    FOR EACH ROW
    EXECUTE FUNCTION maintain_item_references('scroll');

CREATE TRIGGER scroll_reference_cleanup
    AFTER DELETE ON scrolls
    FOR EACH ROW
    EXECUTE FUNCTION cleanup_item_references();

CREATE TRIGGER egg_reference
    AFTER INSERT ON eggs
    FOR EACH ROW
    EXECUTE FUNCTION maintain_item_references('egg');

CREATE TRIGGER creature_reference
    AFTER INSERT ON creatures
    FOR EACH ROW
    EXECUTE FUNCTION maintain_item_references('creature');

CREATE TRIGGER egg_reference_cleanup
    AFTER DELETE ON eggs
    FOR EACH ROW
    EXECUTE FUNCTION cleanup_item_references();

CREATE TRIGGER creature_reference_cleanup
    AFTER DELETE ON creatures
    FOR EACH ROW
    EXECUTE FUNCTION cleanup_item_references();

CREATE OR REPLACE FUNCTION verify_item_type()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.item_type != (
        SELECT item_type 
        FROM item_references 
        WHERE id = NEW.item_id
    ) THEN
        RAISE EXCEPTION 'Item type mismatch';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TABLE user_preferences (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    notification_settings JSONB NOT NULL DEFAULT '{}',
    privacy_settings JSONB NOT NULL DEFAULT '{}',
    ui_preferences JSONB NOT NULL DEFAULT '{}'
);

CREATE TABLE refresh_tokens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    replaced_by_token TEXT,
    ip_address INET
);

CREATE TABLE login_attempts (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    username VARCHAR(50) NOT NULL,
    attempt_time TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    ip_address INET,
    successful BOOLEAN NOT NULL DEFAULT false
);

CREATE TABLE password_resets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token VARCHAR(255) UNIQUE NOT NULL,
    email_code VARCHAR(6),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    code_verified_at TIMESTAMPTZ,
    reset_completed_at TIMESTAMPTZ,
    attempts_remaining INT NOT NULL DEFAULT 3,
    CONSTRAINT valid_email_code CHECK (email_code ~ '^[0-9]{6}$')
);

CREATE TABLE magic_link_tokens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email VARCHAR(100) NOT NULL,
    token VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    token_data JSONB
);

CREATE TABLE item_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    item_id UUID NOT NULL REFERENCES item_references(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    item_type VARCHAR(20) NOT NULL,
    event_type event_type NOT NULL,
    from_user_id UUID REFERENCES users(id),
    to_user_id UUID REFERENCES users(id),
    performed_by_user_id UUID NOT NULL REFERENCES users(id),
    timestamp TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    additional_data JSONB,
    CONSTRAINT valid_item_type CHECK (item_type IN ('egg', 'creature', 'scroll')),
    CONSTRAINT unique_item_timestamp UNIQUE (item_id, timestamp)
);

CREATE TABLE market_listings (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    seller_id UUID NOT NULL REFERENCES users(id),
    item_id UUID NOT NULL REFERENCES item_references(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    item_type VARCHAR(20) NOT NULL,
    price INTEGER NOT NULL CHECK (price > 0),
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    type market_type NOT NULL DEFAULT 'sale',
    status market_status_type NOT NULL DEFAULT 'active',
    buyer_id UUID REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT valid_item_type CHECK (item_type IN ('egg', 'creature', 'scroll'))
);

CREATE TABLE scroll_orderbook (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    side order_side_type NOT NULL,
    price INTEGER NOT NULL CHECK (price > 0 AND price <= 1000000000),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    status market_status_type NOT NULL DEFAULT 'active'
);

CREATE TRIGGER update_scroll_orderbook_timestamp
    BEFORE UPDATE ON scroll_orderbook
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_timestamp();

CREATE INDEX idx_scroll_orderbook_user ON scroll_orderbook(user_id);
CREATE INDEX idx_scroll_orderbook_status ON scroll_orderbook(status);
CREATE INDEX idx_scroll_orderbook_side ON scroll_orderbook(side);
CREATE INDEX idx_scroll_orderbook_price ON scroll_orderbook(price);

CREATE TABLE recipes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    result_item_id UUID NOT NULL REFERENCES scrolls(id) ON DELETE CASCADE,
    ingredients JSONB NOT NULL,
    required_rank user_rank NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE applied_items (
    egg_id UUID REFERENCES eggs(id) ON DELETE CASCADE,
    scroll_id UUID REFERENCES scrolls(id) ON DELETE CASCADE,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (egg_id, scroll_id)
);

CREATE TABLE achievements (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT NOT NULL,
    requirements JSONB NOT NULL,
    rewards JSONB NOT NULL
);

CREATE TABLE user_achievements (
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    achievement_id UUID REFERENCES achievements(id) ON DELETE CASCADE,
    completed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, achievement_id)
);

CREATE OR REPLACE FUNCTION verify_creature_history()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.original_egg_summoned_by != (
        SELECT performed_by_user_id 
        FROM item_events 
        WHERE item_id = NEW.original_egg_id 
        AND event_type = 'summoned' 
        LIMIT 1
    ) THEN
        RAISE EXCEPTION 'Inconsistent egg summoner history';
    END IF;

    IF NEW.hatched_by != (
        SELECT performed_by_user_id 
        FROM item_events 
        WHERE item_id = NEW.id 
        AND event_type = 'hatched' 
        LIMIT 1
    ) THEN
        RAISE EXCEPTION 'Inconsistent hatching history';
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION create_egg_summon_event()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO item_events (
        item_id,
        item_type,
        event_type,
        from_user_id,
        to_user_id,
        performed_by_user_id
    ) VALUES (
        NEW.id,
        'egg',
        'summoned',
        NULL,
        NEW.owner_id,
        NEW.summoned_by
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER verify_creature_history_trigger
    BEFORE INSERT OR UPDATE ON creatures
    FOR EACH ROW
    EXECUTE FUNCTION verify_creature_history();

CREATE TRIGGER egg_summon_event_trigger
    AFTER INSERT ON eggs
    FOR EACH ROW
    EXECUTE FUNCTION create_egg_summon_event();

CREATE TRIGGER update_market_timestamp 
    BEFORE UPDATE ON market_listings
    FOR EACH ROW 
    EXECUTE FUNCTION trigger_set_timestamp();

CREATE TRIGGER update_users_timestamp
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_timestamp();

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_rank ON users(rank);
CREATE INDEX idx_refresh_tokens_user ON refresh_tokens(user_id);
CREATE INDEX idx_refresh_tokens_token ON refresh_tokens(token);
CREATE INDEX idx_refresh_tokens_expires ON refresh_tokens(expires_at);
CREATE INDEX idx_login_attempts_username ON login_attempts(username);
CREATE INDEX idx_login_attempts_time ON login_attempts(attempt_time);
CREATE INDEX idx_login_attempts_ip ON login_attempts(ip_address);
CREATE INDEX idx_password_resets_token ON password_resets(token);
CREATE INDEX idx_password_resets_user ON password_resets(user_id);
CREATE INDEX idx_password_resets_expires ON password_resets(expires_at);
CREATE INDEX idx_password_resets_email_code ON password_resets(email_code);
CREATE INDEX idx_password_resets_user_expires ON password_resets(user_id, expires_at);
CREATE INDEX idx_eggs_owner ON eggs(owner_id);
CREATE INDEX idx_eggs_created ON eggs(created_at);
CREATE INDEX idx_creatures_owner ON creatures(owner_id);
CREATE INDEX idx_creatures_hatched ON creatures(hatched_at);
CREATE INDEX idx_creatures_egg_image ON creatures(original_egg_image_path);
CREATE INDEX idx_creatures_original_egg ON creatures(original_egg_id);
CREATE INDEX idx_creatures_rarity ON creatures(rarity);
CREATE INDEX idx_market_seller ON market_listings(seller_id);
CREATE INDEX idx_market_buyer ON market_listings(buyer_id);
CREATE INDEX idx_market_status ON market_listings(status);
CREATE INDEX idx_market_created ON market_listings(created_at);
CREATE INDEX idx_item_events_composite ON item_events(item_id, item_type);
CREATE INDEX idx_item_events_timestamp ON item_events(timestamp);
CREATE INDEX idx_item_events_type ON item_events(event_type);
CREATE INDEX idx_achievements_completed ON user_achievements(user_id, completed_at);

-- New table for membership codes
CREATE TABLE membership_codes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    code_hash TEXT NOT NULL UNIQUE,
    created_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ,
    used_at TIMESTAMPTZ,
    used_by UUID REFERENCES users(id) DEFERRABLE INITIALLY DEFERRED,
    is_valid BOOLEAN NOT NULL DEFAULT true,
    duration_minutes INTEGER NOT NULL DEFAULT 1
);

-- Table for Patreon supporters
CREATE TABLE patreon_supporters (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    patreon_id VARCHAR(255) UNIQUE NOT NULL,
    full_name VARCHAR(255) NOT NULL,
    email VARCHAR(255) NOT NULL,
    campaign_lifetime_support_cents INTEGER NOT NULL DEFAULT 0,
    currently_entitled_amount_cents INTEGER NOT NULL DEFAULT 0,
    last_charge_date TIMESTAMPTZ,
    last_charge_status VARCHAR(50),
    next_charge_date TIMESTAMPTZ,
    patron_status VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TRIGGER update_patreon_supporters_timestamp
    BEFORE UPDATE ON patreon_supporters
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_timestamp();

-- Add indexes for patreon_supporters table
CREATE INDEX idx_patreon_supporters_patreon_id ON patreon_supporters(patreon_id);
CREATE INDEX idx_patreon_supporters_email ON patreon_supporters(email);
CREATE INDEX idx_patreon_supporters_patron_status ON patreon_supporters(patron_status);

-- Table for Patreon OAuth tokens
CREATE TABLE patreon_tokens (
    patreon_id VARCHAR(255) PRIMARY KEY REFERENCES patreon_supporters(patreon_id) ON DELETE CASCADE,
    access_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    scope TEXT NOT NULL,
    token_type TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Add trigger for patreon_tokens updated_at
CREATE TRIGGER update_patreon_tokens_timestamp
    BEFORE UPDATE ON patreon_tokens
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_timestamp();

-- Add index on expires_at for token refresh job
CREATE INDEX idx_patreon_tokens_expires_at ON patreon_tokens(expires_at);

-- Table for linking users with their Patreon accounts
CREATE TABLE user_patreon_links (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    patreon_id VARCHAR(255) NOT NULL REFERENCES patreon_supporters(patreon_id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(user_id, patreon_id),
    UNIQUE(patreon_id)
);

CREATE TRIGGER update_user_patreon_links_timestamp
    BEFORE UPDATE ON user_patreon_links
    FOR EACH ROW
    EXECUTE FUNCTION trigger_set_timestamp();

CREATE INDEX idx_user_patreon_links_user_id ON user_patreon_links(user_id);
CREATE INDEX idx_user_patreon_links_patreon_id ON user_patreon_links(patreon_id);

-- Create a generic game leaderboard table
CREATE TABLE game_leaderboard (
    game_type TEXT NOT NULL,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    high_score INTEGER NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (game_type, user_id)
);

CREATE TABLE word_game_stats (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    current_streak INT NOT NULL DEFAULT 0,
    highest_streak INT NOT NULL DEFAULT 0,
    last_played_date DATE,
    fastest_time INT,
    total_words_guessed INT NOT NULL DEFAULT 0,
    total_games_played INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_magic_link_tokens_email ON magic_link_tokens(email);
CREATE INDEX idx_magic_link_tokens_token ON magic_link_tokens(token);
CREATE INDEX idx_magic_link_tokens_expires ON magic_link_tokens(expires_at);

-- Magic Button Clicks table
CREATE TABLE magic_button_clicks (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    clicked_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    reward_amount INTEGER NOT NULL,
    CONSTRAINT valid_reward_amount CHECK (reward_amount BETWEEN 1 AND 5)
);

CREATE INDEX idx_magic_button_clicks_time ON magic_button_clicks(clicked_at);
CREATE INDEX idx_magic_button_last_click ON magic_button_clicks(user_id, clicked_at);