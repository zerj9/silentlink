SET search_path = ag_catalog, "$user", public;

-- Create Categories
SELECT * FROM cypher('silentlink_local', $$
CREATE (:Category {name: 'Terrorism'})
CREATE (:Category {name: 'Natural and Environmental Hazards'})
$$) as (v agtype);

-- Create Risks
SELECT * FROM cypher('silentlink_local', $$
CREATE (:Risk {
   name: 'International Terrorist Attack',
   likelihood: 2,
   impact_level: 2
})
CREATE (:Risk {
   name: 'Severe Space Weather',
   likelihood: 4,
   impact_level: 4
})
$$) as (v agtype);

-- Create Impacts
SELECT * FROM cypher('silentlink_local', $$
CREATE (:Impact {
   name: 'Fatalities and Casualties',
   description: 'Direct impact on human life and health'
})
CREATE (:Impact {
   name: 'Economic Damage',
   description: 'Impact on business and financial systems'
})
$$) as (v agtype);

-- Create Capabilities
SELECT * FROM cypher('silentlink_local', $$
CREATE (:Capability {
   name: 'Resilient Communications Systems',
   description: 'Systems that can maintain operation during crisis'
})
CREATE (:Capability {
   name: 'Humanitarian Assistance',
   description: 'Support for affected populations'
})
CREATE (:Capability {
   name: 'Mobile Power Generation',
   description: 'Backup power systems for critical infrastructure'
})
CREATE (:Capability {
   name: 'Emergency Services Response',
   description: 'Coordinated response from emergency services'
})
$$) as (v agtype);

-- Create relationships
SELECT * FROM cypher('silentlink_local', $$
MATCH (r:Risk {name: 'International Terrorist Attack'}), 
     (c:Category {name: 'Terrorism'})
CREATE (r)-[:BELONGS_TO]->(c)
$$) as (v agtype);

SELECT * FROM cypher('silentlink_local', $$
MATCH (r:Risk {name: 'Severe Space Weather'}), 
     (c:Category {name: 'Natural and Environmental Hazards'})
CREATE (r)-[:BELONGS_TO]->(c)
$$) as (v agtype);

SELECT * FROM cypher('silentlink_local', $$
MATCH (r:Risk {name: 'International Terrorist Attack'}), 
     (i:Impact {name: 'Fatalities and Casualties'})
CREATE (r)-[:IMPACTS]->(i)
$$) as (v agtype);

SELECT * FROM cypher('silentlink_local', $$
MATCH (r:Risk {name: 'Severe Space Weather'}), 
     (i:Impact {name: 'Economic Damage'})
CREATE (r)-[:IMPACTS]->(i)
$$) as (v agtype);

SELECT * FROM cypher('silentlink_local', $$
MATCH (r:Risk {name: 'International Terrorist Attack'}), 
     (c:Capability {name: 'Resilient Communications Systems'})
CREATE (r)-[:REQUIRES]->(c)
$$) as (v agtype);

SELECT * FROM cypher('silentlink_local', $$
MATCH (r:Risk {name: 'Severe Space Weather'}), 
     (c:Capability {name: 'Mobile Power Generation'})
CREATE (r)-[:REQUIRES]->(c)
$$) as (v agtype);
