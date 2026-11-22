# context-bench

Local corpus: 6 repositories, 25 questions each. A hit means the answer string is present in the context. This is retrieval coverage, not an LLM accuracy score. The optional judge prompt is `bench/judge_prompt.txt`.

| Method | Avg tokens | Avg unfold tokens | Coverage | Avg ms |
| --- | ---: | ---: | ---: | ---: |
| full dump | 16003 | 0 | 100.0% | 634 |
| repomix-style compress | 12980 | 0 | 100.0% | 145 |
| name map | 1482 | 0 | 0.0% | 438 |
| ast-pack 8000 | 7562 | 0 | 44.0% | 881 |
| ast-pack 8000 + unfold | 7562 | 538 | 100.0% | 2657 |
| ast-pack 16000 | 14706 | 0 | 92.7% | 990 |
| ast-pack 16000 + unfold | 14706 | 79 | 100.0% | 1342 |
| ast-pack 32000 | 16003 | 0 | 100.0% | 979 |
| ast-pack 32000 + unfold | 16003 | 0 | 100.0% | 933 |

Full dump and the repomix-style strip keep every body, so coverage is high and so is the token count. The name map keeps almost none of the answers. ast-pack at a small budget shows signatures and fold ids; unfold spends extra tokens only on the bodies the question names.
