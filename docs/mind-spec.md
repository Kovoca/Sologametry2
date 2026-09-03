# The mind — target specification

Peter's specification, recorded here as the authoritative target. Where
this and the repository disagree, this describes the target and
`docs/status.md` describes what is built.

## The central claim

**A person does not have one "happiness" stat controlling everything.** A
person is an assembly of temperament, beliefs, tastes, recurring needs,
ambitions, relationships, knowledge, immediate emotions, accumulated
stress, memories, social abilities and current circumstances.

These form a loop:

```
perceived event → personal appraisal → emotion → stress + action impulse
                        ↑                  ↓
                   recollection ←────── memory ──→ long-term personality change
                        ↑                  ↓
                        └── relationship & knowledge change ← action
```

The strength of the model is not dialogue. **It is that an event goes on
affecting a person after it is over.**

## The layers, and how fast each moves

| layer | changes | purpose |
|---|---|---|
| species/cultural baseline | generations | starting temperament and values |
| mental attributes | slowly | cognitive capability |
| personality facets | very slowly | behavioural disposition |
| values | slowly | what the person believes |
| preferences | occasionally | specific likes and dislikes |
| goals | rarely | long-term aspirations |
| relationships | gradually / on events | feelings toward particular people |
| knowledge and beliefs | constantly | what they think is true |
| needs | constantly | recurring psychological pressure |
| active emotions | seconds–days | immediate reaction |
| general mood | hours–weeks | bias from recent experience |
| stress | months–years | accumulated burden |
| focus | hours–months | ability to perform |
| memories | experience-dependent | persistent personal history |
| current intention | seconds–hours | what they decided to do |

Keeping them separate is what permits believable contradiction: a dutiful
coward who obeys while terrified; a hot-tempered pacifist who becomes
angry and then regrets it; a greedy person who believes theft is immoral;
a miserable craftsman who stays focused on work; a content person who is
distracted because nothing has engaged them intellectually.

## Sections of the specification

1. The complete mental stack
2. What exists when a person is created — attributes, facets, values,
   preferences, goals
3. Physical drives against psychological needs, and **semantic** need
   satisfaction (standing near somebody is not socialising)
4. **Perception before emotion** — a person reacts only to what they
   perceived or were told, so `WorldEvent` and `PerceivedEvent` are two
   records and the mind appraises the second
5. Emotional appraisal — twelve questions, one event, several emotions
6. Thoughts are not memories
7. Memory: working, episodic, semantic, core; salience, similarity
   suppression, recall triggers, and **recall producing a new appraisal**
8. Personality change — rare, cumulative, mechanism-tagged
9. **Stress, mood and focus are three different things**
10. Coping and breakdown, staged rather than a tantrum table
11. Strange moods as a separate system from stress
12. Relationships as asymmetric, multidimensional records
13. Social opportunity — proximity, schedules and buildings gate contact
14. Choosing a social intention
15. Choosing a topic
16. Social tactics and skills
17. Listener interpretation — the listener's reading, not the speaker's
    intent, is what creates the emotional response
18. Resolving an exchange
19. Requests, persuasion and authority
20. Arguments and value change — doubt before change
21. Knowledge, rumours and lies; **reputation as distributed belief**
22. Action selection outside conversation, with commitment/hysteresis
23. A worked example (the mine collapse)
24. The data model
25. Update frequencies — event-driven wherever possible
26. Scaling: loaded / settlement / distant
27. What to copy from DF and what to improve

## How this is being built

In slices that each land green, the same way Phase 1 is going.

| slice | what | state |
|---|---|---|
| 1 | Static mind: attributes, facets, values. Appraisal producing **several** emotions. Stress, mood and focus as three separate things | **built** |
| 2 | Memory: provenance first — world event / perceived / trace / recollection; routine consolidation; recall producing a *new* appraisal | **built** |
| 3 | Perception driven by the world: line of sight, acoustics, shared contexts, word of mouth | **built** |
| 4 | Needs, semantic satisfaction, focus driven by them | **built** |
| 5 | Relationships as directed multidimensional records; objective ties kept apart; labels derived | **built** |
| 6 | Social opportunity, intention, tactic, interpretation, exchange | **built** |
| 7 | Personality and value change from core memories | **built** |
| 8 | Coping and staged breakdown | **built** |
| 9 | Scaling: what a distant person retains | |

## A note on calibration

This project anchors on measured real-world figures rather than on DF's
numbers, and there is a clean hook: **DF's personality system is itself a
five-factor model**, so the anchor is the Big Five and its measured
distributions rather than DF's tables. Where a figure is available from
psychometrics it is used in preference to a game's.
