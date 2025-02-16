-- Create extension if it doesn't exist
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 
        FROM pg_extension 
        WHERE extname = 'age'
    ) THEN
        CREATE EXTENSION age;
    END IF;
END$$;

CREATE SCHEMA app_data;

-- Trigger to auto-update `updated_at`
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Table to store user information
CREATE TABLE app_data.user (
    id UUID PRIMARY KEY,
    email VARCHAR(255) NOT NULL UNIQUE,
    first_name VARCHAR(100),
    last_name VARCHAR(100),
    created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
    is_active BOOLEAN DEFAULT true
);
CREATE INDEX idx_users_email ON app_data.user(email);

CREATE TRIGGER update_user_updated_at
BEFORE UPDATE ON app_data.user
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

-- Generic federated auth table (for Google, Facebook, etc.)
CREATE TABLE app_data.federated_user (
    id          UUID PRIMARY KEY,
    user_id     UUID NOT NULL REFERENCES app_data.user (id) ON DELETE CASCADE,
    provider    VARCHAR(50) NOT NULL,    -- e.g., 'google', 'microsoft'
    sub         VARCHAR(255) NOT NULL,   -- "Subject" (unique ID) from the provider
    email       VARCHAR(255),            -- Email as reported by the provider
    picture_url VARCHAR(255),            -- Profile image from the provider
    created_at  TIMESTAMPTZ DEFAULT now(),
    updated_at  TIMESTAMPTZ DEFAULT now(),

    -- Ensures a single (provider, sub) combination is unique
    UNIQUE (provider, sub)
);
CREATE INDEX idx_federated_user_provider_email ON app_data.federated_user(email);

-- Table to store session information
CREATE TABLE app_data.session (
    id                 UUID PRIMARY KEY,
    user_id            UUID NOT NULL REFERENCES app_data.user (id) ON DELETE CASCADE,
    federated_user_id  UUID REFERENCES app_data.federated_user (id) ON DELETE CASCADE,

    refresh_token      VARCHAR(512),
    token_expiry       TIMESTAMPTZ,
    session_expiry     TIMESTAMPTZ,

    -- Additional session metadata
    device_info        VARCHAR(255),          -- e.g. 'Chrome on Windows 10'
    ip_address         INET,                  -- or VARCHAR(45) for IPv6
    created_at         TIMESTAMPTZ DEFAULT now(),
    updated_at         TIMESTAMPTZ DEFAULT now()
);

-- Table to store OAuth states
-- TODO: Store path to redirect to after OAuth flow
CREATE TABLE app_data.oauth_session (
    state TEXT PRIMARY KEY, -- The csrf state value
    nonce TEXT NOT NULL, -- The nonce value
    pkce_verifier TEXT NOT NULL, -- The PKCE verifier value
    created_at TIMESTAMPTZ DEFAULT NOW(), -- When the state was created
    expires_at TIMESTAMPTZ NOT NULL -- When the state expires
);


-- Table to store organization information
CREATE TABLE app_data.org (
    id integer PRIMARY KEY,
    name text NOT NULL,
    description text,
    admin UUID NOT NULL REFERENCES app_data.user(id),
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    CHECK (name <> '')
);

CREATE TRIGGER update_org_updated_at
BEFORE UPDATE ON app_data.org
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

-- TODO: Add org as a foreign key
-- Table to store graph information
CREATE TABLE app_data.graph_info (
    -- Graph id created by the application, must start with _ or letter to pass to AGE
    app_graphid text PRIMARY KEY,
    -- Original graphid created by AGE, used to query the graph
    age_graphid integer NOT NULL REFERENCES ag_catalog.ag_graph(graphid),  
    name text NOT NULL,
    description text,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    UNIQUE (app_graphid),
    UNIQUE (age_graphid),
    CHECK (name <> '')
);


CREATE INDEX idx_app_graphid ON app_data.graph_info (app_graphid);
CREATE INDEX idx_age_graphid ON app_data.graph_info (age_graphid);

-- Table to store user and organization relationship
CREATE TABLE app_data.user_org (
    user_id UUID NOT NULL REFERENCES app_data.user(id),
    org_id integer NOT NULL REFERENCES app_data.org(id),
    role text NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (user_id, org_id),
    CHECK (role <> '')
);

-- Table to store graph and user relationship
CREATE TABLE app_data.graph_user (
    app_graphid text NOT NULL REFERENCES app_data.graph_info(app_graphid),
    user_id UUID NOT NULL REFERENCES app_data.user(id),
    role text NOT NULL,
    created_at TIMESTAMPTZ DEFAULT now(),
    updated_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (app_graphid, user_id),
    CHECK (role <> '')
);
