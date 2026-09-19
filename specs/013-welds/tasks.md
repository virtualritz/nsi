# Tasks: Weld Declarations

- [x] T1 `EdgeKind::Weld`.
- [x] T2 `Scene::weld_table`: parse and validate. *Evidence:* the weld tests.
- [x] T3 `Scene::welds`: group by `(weld, id)`. *Evidence:* falsified grouping test.
- [x] T4 Diagnostics as data, one test per invalid case, falsified.
- [x] T5 Stream round trip. *Evidence:* `a_weld_table_round_trips`.
- [x] T6 `affected` covers both sides of a seam.
- [ ] T7 A test for T6.
- [ ] T8 Occurrence scope under instancing, once the draft settles it.
