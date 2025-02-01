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

CREATE TABLE app_data.user (
    id text PRIMARY KEY,
    email VARCHAR(255) NOT NULL UNIQUE,
    first_name VARCHAR(100),
    last_name VARCHAR(100),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    is_active BOOLEAN DEFAULT true
);

CREATE INDEX idx_users_email ON app_data.user(email);

-- Trigger to auto-update `updated_at`
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER update_user_updated_at
BEFORE UPDATE ON app_data.user
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

-- Generic federated auth table (for Google, Facebook, etc.)
CREATE TABLE app_data.federated_auth (
    user_id text REFERENCES app_data.user(id) ON DELETE CASCADE,
    provider VARCHAR(50) NOT NULL, -- Federated provider (e.g., 'google', 'microsoft')
    provider_user_id VARCHAR(255) NOT NULL,
    provider_email VARCHAR(255) NOT NULL,
    access_token VARCHAR(512),
    refresh_token VARCHAR(512),
    token_expires_at TIMESTAMP WITH TIME ZONE,
    picture_url VARCHAR(255),
    PRIMARY KEY (user_id, provider)
);

-- Table to store OAuth states
-- TODO: Store path to redirect to after OAuth flow
CREATE TABLE app_data.oauth_states (
    state TEXT PRIMARY KEY,          -- The CSRF state value
    nonce TEXT NOT NULL,             -- The nonce value
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(), -- When the state was created
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL       -- When the state expires
);

-- Indexes
CREATE INDEX idx_federated_auth_provider_user_id ON app_data.federated_auth(provider_user_id);
CREATE INDEX idx_federated_auth_provider_email ON app_data.federated_auth(provider_email);

-- Table to store organization information
CREATE TABLE app_data.org (
    id integer PRIMARY KEY,
    name text NOT NULL,
    description text,
    admin text NOT NULL REFERENCES app_data.user(id),
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
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
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    UNIQUE (app_graphid),
    UNIQUE (age_graphid),
    CHECK (name <> '')
);


CREATE INDEX idx_app_graphid ON app_data.graph_info (app_graphid);
CREATE INDEX idx_age_graphid ON app_data.graph_info (age_graphid);

-- Table to store user and organization relationship
CREATE TABLE app_data.user_org (
    user_id text NOT NULL REFERENCES app_data.user(id),
    org_id integer NOT NULL REFERENCES app_data.org(id),
    role text NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    PRIMARY KEY (user_id, org_id),
    CHECK (role <> '')
);

-- Table to store graph and user relationship
CREATE TABLE app_data.graph_user (
    app_graphid text NOT NULL REFERENCES app_data.graph_info(app_graphid),
    user_id text NOT NULL REFERENCES app_data.user(id),
    role text NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    PRIMARY KEY (app_graphid, user_id),
    CHECK (role <> '')
);


-- Table to store session information
CREATE TABLE app_data.sessions (
    id UUID PRIMARY KEY,             -- Unique session ID
    user_id TEXT NOT NULL REFERENCES app_data.user(id) ON DELETE CASCADE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(), -- Session creation time
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL       -- Session expiry time
);
