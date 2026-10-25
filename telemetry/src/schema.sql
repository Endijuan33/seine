CREATE TABLE IF NOT EXISTS telemetry_reports (
    report_id text PRIMARY KEY,
    installation_id text NOT NULL,
    received_at timestamptz NOT NULL DEFAULT now(),
    payload jsonb NOT NULL
);
CREATE INDEX IF NOT EXISTS telemetry_reports_received ON telemetry_reports (received_at);
CREATE INDEX IF NOT EXISTS telemetry_reports_installation ON telemetry_reports (installation_id, received_at DESC);

-- One latest report per installation prevents uptime from skewing popularity.
CREATE OR REPLACE VIEW telemetry_installations AS
SELECT DISTINCT ON (installation_id) installation_id, received_at, payload
FROM telemetry_reports ORDER BY installation_id, received_at DESC;

CREATE OR REPLACE VIEW telemetry_cpu_population AS
SELECT payload #>> '{hardware,cpu_model}' AS cpu_model,
       payload #>> '{hardware,os}' AS os,
       count(*) AS installations
FROM telemetry_installations
GROUP BY 1, 2;

-- Per-installation medians first: frequent reporters cannot dominate results.
CREATE OR REPLACE VIEW telemetry_backend_performance AS
WITH samples AS (
    SELECT installation_id, payload->>'miner_version' AS miner_version,
           payload #>> '{hardware,cpu_model}' AS cpu_model,
           payload #>> '{hardware,os}' AS os, sample->'settings' AS settings,
           (sample->>'hashes')::double precision / NULLIF((sample->>'observed_secs')::double precision, 0) AS hps
    FROM telemetry_reports CROSS JOIN LATERAL jsonb_array_elements(payload->'backends') AS sample
    WHERE NOT (payload->>'debug_build')::boolean
      AND NOT (sample->>'settings_changed')::boolean
      AND (sample->>'observed_secs')::double precision >= 60
), per_installation AS (
    SELECT installation_id, miner_version, cpu_model, os, settings,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY hps) AS hps
    FROM samples GROUP BY 1, 2, 3, 4, 5
)
SELECT miner_version, cpu_model, os, settings, count(*) AS installations,
       percentile_cont(0.5) WITHIN GROUP (ORDER BY hps) AS median_hps,
       percentile_cont(0.1) WITHIN GROUP (ORDER BY hps) AS p10_hps,
       percentile_cont(0.9) WITHIN GROUP (ORDER BY hps) AS p90_hps
FROM per_installation GROUP BY 1, 2, 3, 4;
