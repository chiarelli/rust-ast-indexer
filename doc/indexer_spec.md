# Indexer specification — Git-based incremental indexing

This document describes the behavior of the `incremental_index` command and the Git integration used by the indexer (`crate::infra::git`). It specifies payload fields, expected semantics, events emitted by the CLI, error handling, and examples.

## Purpose

Enable incremental indexing driven by Git metadata so callers can request indexing only for files tracked by Git or for files changed between two refs. This reduces work and enables fast CI/agent workflows.

## Command: `incremental_index`

The CLI accepts a JSONL `Command` whose `command` field equals `incremental_index`. The `payload` object supports the following fields:

- `path` (string, required): repository/workspace path where the indexer should run.
- `use_git` (bool, optional, default: false): if true, the CLI will consult Git to discover the set of files to index instead of scanning the filesystem.
- `git_range` (object, optional): when present and `use_git=true`, it should contain `from` and `to` strings (refs / tags / commits) describing the git range to diff. If both `from` and `to` are present, the indexer will index files reported by `git diff --name-only <from> <to>`.
- `files` (array[string], optional): explicit list of file paths to index. Used when `use_git=false` or as an explicit override.
- `options` (object, optional): indexer options, such as `max_concurrency`.

Example payloads:

- Full tracked files:

```json
{"path":"/repo", "use_git":true}
```

- Git diff between tags:

```json
{"path":"/repo", "use_git":true, "git_range": {"from":"v1","to":"HEAD"}}
```

- Explicit file list (no git):

```json
{"path":"/repo", "files":["src/lib.rs","README.md"], "options":{"max_concurrency":4}}
```

## Behavior and semantics

1. Validate `payload.path` is present and non-empty. If missing, emit an `error` event with code `INVALID_PAYLOAD` and abort the job.
2. Determine file set:
   - If `use_git=true` and `git_range` contains both `from` and `to`, call `infra::git::get_git_diff_files(path, from, to)` to obtain the list of files changed between refs.
   - If `use_git=true` and `git_range` is absent or invalid, call `infra::git::emit_git_tracked_files(path)` to obtain all tracked files.
   - If `use_git=false` and the payload contains `files`, use that explicit list.
   - Otherwise, fall back to a full filesystem scan using `walk_path`.
3. If the `infra::git` call returns an error, emit an `error` event with code `GIT_ERROR`, include a helpful `message` describing the failure, then emit `job_completed` with `processed: 0` and `errors: 1`.
4. Construct an `IndexOptions` instance including `explicit_files: Option<Vec<String>>` when the file list was obtained from Git or from the payload.
5. Emit `job_started` event (with `job_id` when present) before indexing, and stream `file_listed` events for each discovered file (see `emit_file_listed_from_records` for the explicit-files case or `emit_file_listed_events` for filesystem scan).
6. Run indexer (`index_path_parallel`) with provided options. The indexer will emit `chunk_emitted` events as chunks are processed, and a final `job_completed` event with `processed` and `duration_ms`.

## Error handling

- Missing `git` binary or `not a git repository` errors are surfaced as `GIT_ERROR` with `recoverable:false` and will end the job early.
- Invalid `git_range` shapes fall back to tracked files, but if `emit_git_tracked_files` fails the job will error as above.
- Any `WalkerError` returned by the scanning/walking layer is reported as `WALKER_ERROR` and will result in `job_completed` with `errors:1`.

## Event stream contract

The command should produce a deterministic sequence of events on success:

1. `job_started` { job_id }
2. multiple `file_listed` events (stream)
3. multiple `chunk_emitted` events (stream)
4. `job_completed` { processed, duration_ms }

On Git errors, the sequence is:

1. `error` { code: "GIT_ERROR", message }
2. `job_completed` { processed: 0, errors: 1 }

## Notes and limitations

- File paths returned by the Git helper are relative to the repository root and are consumed as-is by the indexer. Callers should ensure the `path` argument is the repository root (or an appropriate subpath) to produce correct relative paths.
- The current implementation uses the system `git` binary. CI environments must provide `git` in PATH for `use_git=true` flows and for unit tests that exercise `infra::git`.
- For simplicity the first iteration uses placeholders for `FileRecord` metadata when explicit file lists are provided. The indexer computes or fills missing metadata during processing.

## Examples

- Request incremental indexing of files changed since tag `v1`:

```json
{
  "command": "incremental_index",
  "job_id": "job-123",
  "payload": { "path": ".", "use_git": true, "git_range": { "from": "v1", "to": "HEAD" } }
}
```

- Request indexing of tracked files only:

```json
{
  "command": "incremental_index",
  "payload": { "path": ".", "use_git": true }
}
```

## Next steps

- Add an integration smoke test (CI) that creates a temporary git repo, commits files, modifies them, and executes the binary with `incremental_index` payload to validate the end-to-end behavior. This will be implemented as the next task.
