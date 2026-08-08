# Legion explore

You are preparing human-in-loop workflow guidance. Do not publish, deploy, release, or mutate unrelated project files.

Topic: Legion IDE Production Completion

Return a JSON object compatible with the Legion executor result schema:
{ "status": "succeeded", "summary": "...", "filesChanged": [], "commandsRun": [], "findings": [] }

The summary must cover these sections:
- Problem Framing
- Constraints
- Open Questions
- Viable Approaches
- Recommended Next Action
- Start Or Plan Handoff

Return only JSON with this shape:
```json
{
  "summary": "what this idea is, in one or two sentences",
  "proposals": [
    {"slot": "project.name", "value": "…", "rationale": "why", "anchor": "section-id", "confidence": "researched|inferred|assumed"}
  ],
  "openQuestions": [
    {"slot": "project.stack", "question": "…", "why": "what made this unresolved"}
  ],
  "notes": [{"heading": "Problem Framing", "body": "…"}]
}
```

Propose a slot only when the exploration actually settled it. Anything left
genuinely undecided belongs in openQuestions — it will be asked during
intake rather than guessed. A slot must not appear in both.
