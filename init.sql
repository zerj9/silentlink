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

SELECT ag_catalog.create_graph('silentlink_local');

-- Create node labels
SELECT ag_catalog.create_vlabel('silentlink_local', 'Risk');
SELECT ag_catalog.create_vlabel('silentlink_local', 'Category');
SELECT ag_catalog.create_vlabel('silentlink_local', 'Impact');
SELECT ag_catalog.create_vlabel('silentlink_local', 'Capability');

-- Create edge labels
SELECT ag_catalog.create_elabel('silentlink_local', 'BELONGS_TO');
SELECT ag_catalog.create_elabel('silentlink_local', 'IMPACTS');
SELECT ag_catalog.create_elabel('silentlink_local', 'REQUIRES');
SELECT ag_catalog.create_elabel('silentlink_local', 'RELATED_TO');
