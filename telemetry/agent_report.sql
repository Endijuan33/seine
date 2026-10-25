BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SET LOCAL statement_timeout = '15s';
SET LOCAL lock_timeout = '2s';
SET LOCAL TIME ZONE 'America/New_York';

WITH reports AS (
    SELECT installation_id, received_at,
           payload->>'miner_version' AS version,
           payload #>> '{hardware,cpu_model}' AS cpu,
           payload #>> '{hardware,os}' AS os,
           payload->'backends' AS backends
    FROM telemetry_reports
    WHERE received_at >= now() - interval '7 days'
      AND NOT (payload->>'debug_build')::boolean
), latest AS (
    SELECT DISTINCT ON (installation_id) * FROM reports
    ORDER BY installation_id, received_at DESC
), population AS (
    SELECT cpu, os, count(*) AS installations FROM latest
    GROUP BY 1, 2 ORDER BY installations DESC, cpu, os LIMIT 20
), versions AS (
    SELECT version, count(*) AS installations FROM latest
    GROUP BY 1 ORDER BY installations DESC, version
), gpus AS (
    SELECT sample #>> '{settings,backend}' AS backend,
           coalesce(sample #>> '{settings,device_model}', 'unknown') AS gpu,
           count(DISTINCT installation_id) AS installations
    FROM latest CROSS JOIN LATERAL jsonb_array_elements(backends) AS sample
    WHERE sample #>> '{settings,backend}' IN ('nvidia', 'amd', 'metal')
    GROUP BY 1, 2 ORDER BY installations DESC, backend, gpu LIMIT 20
), daily AS (
    SELECT received_at::date AS day, count(*) AS reports,
           count(DISTINCT installation_id) AS installations
    FROM reports GROUP BY 1 ORDER BY 1
), samples AS (
    SELECT installation_id, received_at, version, cpu, os,
           sample->'settings' AS settings,
           (sample->>'hashes')::double precision /
             NULLIF((sample->>'observed_secs')::double precision, 0) AS hps
    FROM reports CROSS JOIN LATERAL jsonb_array_elements(backends) AS sample
    WHERE NOT (sample->>'settings_changed')::boolean
      AND (sample->>'observed_secs')::double precision >= 60
), per_report AS (
    -- Identical devices in one report must not inflate independent observations.
    SELECT installation_id, received_at, version, cpu, os, settings,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY hps) AS hps
    FROM samples GROUP BY 1, 2, 3, 4, 5, 6
), per_installation AS (
    SELECT installation_id, version, cpu, os, settings,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY hps) AS hps
    FROM per_report GROUP BY 1, 2, 3, 4, 5
    HAVING count(*) >= 3 AND max(received_at) - min(received_at) >= interval '24 hours'
), performance AS (
    SELECT version, cpu, os, settings, count(*) AS installations,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY hps) AS median_hps,
           percentile_cont(0.1) WITHIN GROUP (ORDER BY hps) AS p10_hps,
           percentile_cont(0.9) WITHIN GROUP (ORDER BY hps) AS p90_hps
    FROM per_installation GROUP BY 1, 2, 3, 4
    HAVING count(*) >= 5
    ORDER BY installations DESC, version, cpu, os, settings LIMIT 20
)
SELECT jsonb_build_object(
    'window_days', 7,
    'reports', (SELECT count(*) FROM reports),
    'installations', (SELECT count(*) FROM latest),
    'active_installations_24h', (SELECT count(*) FROM latest WHERE received_at >= now() - interval '24 hours'),
    'minutes_since_last_report', (SELECT floor(extract(epoch FROM now() - max(received_at))/60) FROM reports),
    'versions', coalesce((SELECT jsonb_agg(versions) FROM versions), '[]'::jsonb),
    'cpu_population', coalesce((SELECT jsonb_agg(population) FROM population), '[]'::jsonb),
    'gpu_population', coalesce((SELECT jsonb_agg(gpus) FROM gpus), '[]'::jsonb),
    'daily_activity', coalesce((SELECT jsonb_agg(daily) FROM daily), '[]'::jsonb),
    'performance_candidates', coalesce((SELECT jsonb_agg(performance) FROM performance), '[]'::jsonb)
);
ROLLBACK;
