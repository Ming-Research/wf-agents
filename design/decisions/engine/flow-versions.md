# The versions of the flows

Which version of its flow a work runs on, and how what it recorded on an earlier one is read.

### decision a-version-is-kept-as-its-files
A version of a flow is kept once, as the files of its definition, named by the digest of their text, and is compiled
again where a work that ended on it is read.
Why: The compiled flow is the engine's own structure and changes with its code: kept, it would be a recorded form
that every change of the code must migrate (principle generalise-at-the-second-use), while the files are the form the
owner writes, which the compiler goes on reading in every layout it has read. Keeping the compiled flow was turned
down for that.

### decision the-records-say-how-they-read
A work put on another version of its flow records, with the version, each cell its records named before that the
version names otherwise, with the id it is read by there; the records keep every cell as it was recorded, and are
read through each such entry after them, in turn. The version a work is added on is recorded at the place before
every other entry of it.
Why: The ids could be worked out again from the versions kept and their tables of earlier ids, but recorded, they let
the records alone say how they read: the store, and an agent reading it with `query`, read a run as the cell it ran at
without compiling a flow. Writing the new ids over the cells recorded was turned down, as records are only added to
(decision tables-only-added-to).

### decision a-work-that-ended-stays-on-its-version
A work that ended is held, read and drawn on the version it ended on; a work going on is put on the version installed
as the engine starts, and a work reopened as it is reopened. The rounds of a work that ended on another version are
judged in the history of its flow as the cells they are on the version installed.
Why: Read on the version installed, a work that ended would be read by a flow it never ran on (principle
the-past-as-it-ran), while one going on runs on the one flow there is (principle one-flow). A cell's history is its
rounds on every version, under whatever id it had on each (principle judged-by-own-history).

### decision a-work-kept-before-the-versions-takes-the-one-installed
A work kept before the versions were, one that ended too, is put on the version installed as the engine first starts
with them, every cell it recorded read as recorded before any renumbering.
Why: Nothing kept says which version it ran on, and no table of earlier ids was written before the versions were, so
the version installed then is the nearest reading there is; left on none, such a work would be read by whatever flow
is installed at each start.

### decision a-flow-that-rewrites-its-table-is-refused
The engine does not start on a flow whose table of earlier ids does not begin with the whole table of a version a
work ran on, nor while a version a work ended on does not compile.
Why: What such a work recorded could not then be read as the cells it ran at, and the work would be shown otherwise
than it ran (principle nothing-passes-for-normal); a changed flow that fails a check never reaches a work (principle
compile-before-run). Holding such a work at fault and starting all the same was turned down: the fault is the flow's
or the compiler's, and is mended there, not in the work.
