-- AI pipelines (PLAN §9.2–§9.4, §9.8; docs/ARCHITECTURE.md "AI pipelines"): what the correction
-- loop needs to know about each AI decision besides its source and target.
--
-- `rel_type`: the relation key the decision wrote or proposed (`related`, `people`, `works-at`,
--             `concepts`, …), or the custody event type (`stored-at`, …).
-- `mention`:  the exact text the decision was about ("Ahmed", "بابا"), for entity mentions and
--             custody participants; shown to the correction prompt.
-- `detail`:   MessagePack with the decision's remaining context (reason, the entity-link target
--             kind, the custody event), empty when there is none.
--
-- The decision a suggestion came from is looked up by `suggestion_id` (threaded replies,
-- accepting and rejecting).

ALTER TABLE ai_decisions
    ADD COLUMN rel_type text,
    ADD COLUMN mention  text,
    ADD COLUMN detail   bytea NOT NULL DEFAULT '\x'::bytea;

CREATE INDEX ai_decisions_suggestion ON ai_decisions (user_id, suggestion_id)
    WHERE suggestion_id IS NOT NULL;
CREATE INDEX ai_decisions_job ON ai_decisions (user_id, job_id) WHERE job_id IS NOT NULL;
