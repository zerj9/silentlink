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

-- Table to store user information
CREATE TABLE app_data.user_info (
    userid integer PRIMARY KEY,
    first_name text,
    last_name text,
    email text NOT NULL,
    password text NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now(),
    UNIQUE (email),
    CHECK (userid > 0),
    CHECK (email <> ''),
    CHECK (password <> '')
);

CREATE INDEX idx_email ON app_data.user_info (email);
