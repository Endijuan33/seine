-- Paired CPU trial screening. No 24-hour residence requirement; no automatic defaults.
BEGIN READ ONLY;
SET LOCAL statement_timeout = '15s';
SET LOCAL lock_timeout = '2s';
SET LOCAL TIME ZONE 'America/New_York';
WITH reports AS (
    SELECT installation_id, report_id, received_at, payload
    FROM telemetry_reports
    WHERE received_at >= now() - interval '7 days'
      AND NOT (payload->>'debug_build')::boolean
      AND payload->'tuning' IS NOT NULL AND payload->'tuning' <> 'null'::jsonb
), raw AS (
    SELECT installation_id, received_at, payload,
           payload #>> '{tuning,run_id}' AS run_id,
           (trial->>'trial_id')::bigint AS trial_id, trial->'outcome' AS outcome,
           jsonb_build_object(
               'hardware', (payload->'hardware') - 'resources',
               'build_target', payload->'build_target', 'build_features', payload->'build_features',
               'source_fingerprint', payload #> '{diagnostics,source_fingerprint}',
               'mode', payload #> '{diagnostics,mode}'
           ) AS environment
    FROM reports CROSS JOIN LATERAL jsonb_array_elements(payload #> '{tuning,trials}') trial
), identities AS (
    SELECT installation_id, run_id, trial_id,
           count(DISTINCT jsonb_build_array(outcome, environment)) AS variants
    FROM raw GROUP BY 1,2,3
), trials AS (
    -- Conflicting replays are excluded entirely, not resolved by choosing a winner.
    -- Earliest receipt preserves freshness; replay cannot make an old trial new.
    SELECT DISTINCT ON (r.installation_id, r.run_id, r.trial_id) r.*,
           jsonb_build_object('environment', environment, 'kind', outcome->'kind',
               'policy', outcome->'policy', 'baseline', outcome->'baseline',
               'candidate', outcome->'candidate',
               'resources', (outcome #> '{context_before,resources}') - 'available_memory_mib'
           ) AS context
    FROM raw r JOIN identities i USING (installation_id, run_id, trial_id)
    WHERE i.variants = 1
    ORDER BY r.installation_id, r.run_id, r.trial_id, received_at
), windows AS (
    SELECT t.*, position, sample,
           (sample->>'hashes')::bigint AS hashes,
           (sample->>'elapsed_ms')::bigint AS elapsed_ms,
           (sample->>'hashes')::double precision / NULLIF((sample->>'elapsed_ms')::double precision, 0) AS rate,
           ((sample->>'hashes')::bigint >= greatest(1, (outcome #>> '{policy,minimum_hashes}')::bigint)
            AND (sample->>'elapsed_ms')::bigint >= (outcome #>> '{policy,minimum_sample_ms}')::bigint
            OR coalesce(
                (sample->>'hashes')::bigint >= (outcome #>> '{policy,short_minimum_hashes}')::bigint
                AND (sample->>'elapsed_ms')::bigint >= (outcome #>> '{policy,short_sample_ms}')::bigint,
                false)) AS sufficient,
           (NOT (sample->>'changed')::boolean
            AND sample->>'arm' = CASE WHEN position % 3 = 2 THEN 'candidate' ELSE 'baseline' END
            AND sample #> '{context,members}' = CASE WHEN position % 3 = 2 THEN outcome->'candidate' ELSE outcome->'baseline' END
            AND (sample #> '{context,resources}') - 'available_memory_mib'
                = (outcome #> '{context_before,resources}') - 'available_memory_mib') AS stable
    FROM trials t CROSS JOIN LATERAL jsonb_array_elements(outcome->'samples') WITH ORDINALITY s(sample, position)
), health AS (
    SELECT installation_id, run_id, trial_id, count(*) AS windows,
           bool_and(stable) AS stable,
           bool_and(sufficient) AS sufficient
    FROM windows GROUP BY 1,2,3
), cycles AS (
    SELECT a.installation_id, a.run_id, a.trial_id,
           b.rate / NULLIF((a.hashes + c.hashes)::double precision / (a.elapsed_ms + c.elapsed_ms), 0) AS ratio,
           abs(c.rate / NULLIF(a.rate, 0) - 1) AS drift,
           CASE WHEN a.outcome #>> '{policy,quantization}' = 'three_window_hash'
                THEN (1.0/a.elapsed_ms + 1.0/b.elapsed_ms + 1.0/c.elapsed_ms)
                    / NULLIF((a.hashes + c.hashes)::double precision / (a.elapsed_ms + c.elapsed_ms), 0)
                ELSE 0 END AS quantization_ratio
    FROM windows a JOIN windows b
      ON (a.installation_id,a.run_id,a.trial_id) = (b.installation_id,b.run_id,b.trial_id)
     AND b.position = a.position + 1
    JOIN windows c
      ON (a.installation_id,a.run_id,a.trial_id) = (c.installation_id,c.run_id,c.trial_id)
     AND c.position = a.position + 2
    WHERE a.position % 3 = 1
), measured AS (
    SELECT installation_id, run_id, trial_id, count(*) AS cycles,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY ratio) AS ratio,
           min(ratio) AS minimum_cycle_ratio, max(ratio) AS maximum_cycle_ratio,
           max(drift) AS maximum_baseline_drift, max(quantization_ratio) AS maximum_quantization_ratio
    FROM cycles GROUP BY 1,2,3
), qualified AS (
    SELECT t.*, m.ratio, m.minimum_cycle_ratio, m.maximum_cycle_ratio, m.maximum_baseline_drift, m.maximum_quantization_ratio
    FROM trials t JOIN health h USING (installation_id, run_id, trial_id)
    JOIN measured m USING (installation_id, run_id, trial_id)
    WHERE outcome->>'decision' IN ('gain','no_confirmed_gain')
      AND outcome->>'reason' = 'completed'
      AND h.windows IN (3,6) AND m.cycles * 3 = h.windows AND h.stable
      AND h.sufficient
      AND m.maximum_baseline_drift <= (outcome #>> '{policy,maximum_baseline_drift_bps}')::double precision / 10000
      AND m.ratio IS NOT NULL AND m.ratio > 0
      AND outcome->'baseline' <> outcome->'candidate'
      AND outcome #> '{context_before,members}' = outcome->'baseline'
      AND outcome #> '{context_after,members}' = CASE outcome->>'applied'
          WHEN 'candidate' THEN outcome->'candidate' WHEN 'baseline' THEN outcome->'baseline' END
      AND (outcome #> '{context_before,resources}') - 'available_memory_mib'
          = (outcome #> '{context_after,resources}') - 'available_memory_mib'
      AND outcome #>> '{context_before,resources,placement_fingerprint}' IS NOT NULL
      AND outcome #>> '{context_before,resources,allowed_logical_cpus}' IS NOT NULL
      AND outcome #>> '{context_before,resources,available_parallelism}' IS NOT NULL
      AND outcome #>> '{context_before,resources,effective_memory_mib}' IS NOT NULL
      AND environment->>'source_fingerprint' IS NOT NULL
      AND environment->>'source_fingerprint' <> 'unknown'
      AND environment->>'mode' IN ('pool','daemon')
), installations AS (
    SELECT context, installation_id, count(*) AS trials,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY ratio) AS ratio,
           min(minimum_cycle_ratio) AS minimum_cycle_ratio,
           max(maximum_cycle_ratio) AS maximum_cycle_ratio,
           max(maximum_baseline_drift) AS maximum_baseline_drift,
           max(maximum_quantization_ratio) AS maximum_quantization_ratio,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY ratio - 1 - maximum_quantization_ratio
               - (outcome #>> '{policy,required_gain_bps}')::double precision / 10000) AS gain_above_resolution,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY
               (outcome->>'finished_ms')::bigint - (outcome->>'started_ms')::bigint) AS median_wall_ms,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY (outcome #>> '{cost,idle_ms}')::bigint) AS median_idle_ms,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY (outcome #>> '{cost,switching_ms}')::bigint) AS median_switching_ms,
           count(*) FILTER (WHERE outcome #>> '{cost,idle_ms}' IS NOT NULL) AS trials_with_idle_measurement,
           count(*) FILTER (WHERE outcome #>> '{cost,switching_ms}' IS NOT NULL) AS trials_with_switch_measurement,
           min(received_at) AS first_received, max(received_at) AS last_received
    FROM qualified GROUP BY 1,2
), cohorts AS (
    SELECT context, count(*) AS installations, sum(trials) AS qualified_trials,
           count(*) FILTER (WHERE ratio > 1) AS installations_with_positive_ratio,
           count(*) FILTER (WHERE gain_above_resolution > 0) AS installations_with_resolved_gain,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY ratio) AS median_ratio,
           percentile_cont(0.1) WITHIN GROUP (ORDER BY ratio) AS p10_ratio,
           percentile_cont(0.9) WITHIN GROUP (ORDER BY ratio) AS p90_ratio,
           min(minimum_cycle_ratio) AS minimum_cycle_ratio, max(maximum_cycle_ratio) AS maximum_cycle_ratio,
           max(maximum_baseline_drift) AS maximum_baseline_drift,
           max(maximum_quantization_ratio) AS maximum_quantization_ratio,
           sum(trials_with_idle_measurement) AS trials_with_idle_measurement,
           sum(trials_with_switch_measurement) AS trials_with_switch_measurement,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY median_wall_ms) AS median_wall_ms,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY median_idle_ms) AS median_idle_ms,
           percentile_cont(0.5) WITHIN GROUP (ORDER BY median_switching_ms) AS median_switching_ms,
           min(first_received) AS first_received, max(last_received) AS last_received
    FROM installations GROUP BY 1
), hints AS (
    SELECT c.*, false AS automatic_default_eligible, 'requires_native_validation' AS status,
           (SELECT jsonb_agg(DISTINCT jsonb_build_object('version', payload->'miner_version',
               'build_commit', payload #> '{diagnostics,build_commit}')) FROM qualified q
               WHERE q.context = c.context) AS provenance
    FROM cohorts c WHERE installations >= 5
), outcomes AS (
    SELECT outcome->>'kind' AS kind, outcome->>'decision' AS decision,
           outcome->>'reason' AS reason, count(*) AS trials,
           count(DISTINCT installation_id) AS installations
    FROM trials GROUP BY 1,2,3
)
SELECT jsonb_build_object(
    'window_days', 7, 'status', CASE WHEN EXISTS (SELECT 1 FROM trials) THEN 'ok' ELSE 'unavailable' END,
    'reporting_installations', (SELECT count(DISTINCT installation_id) FROM reports),
    'unique_trials', (SELECT count(*) FROM trials),
    'qualified_trials', (SELECT count(*) FROM qualified),
    'conflicting_trial_identities', (SELECT count(*) FROM identities WHERE variants > 1),
    'reported_dropped_trials', coalesce((SELECT sum((payload #>> '{tuning,dropped_trials}')::bigint) FROM reports), 0),
    'outcomes', coalesce((SELECT jsonb_agg(outcomes ORDER BY kind, decision, reason) FROM outcomes), '[]'::jsonb),
    'cohorts_below_coverage', (SELECT count(*) FROM cohorts WHERE installations < 5),
    'ranked_hints', coalesce((SELECT jsonb_agg(h) FROM (SELECT * FROM hints ORDER BY
        (median_ratio > 1 AND installations_with_resolved_gain >= ceil(installations * .6)) DESC,
        median_ratio DESC, installations DESC, last_received DESC LIMIT 30) h), '[]'::jsonb),
    'interpretation', 'Installation-weighted same-context A/B/A observations, not independent-host proof or automatic defaults. Percentiles and cycle ranges describe spread, not confidence intervals. No-confirmed-gain is not a regression. Unknown, changed, aborted and inconclusive trials remain in coverage but cannot rank hints. Validate on matching native hardware before shipping hints with a release.'
);
ROLLBACK;
