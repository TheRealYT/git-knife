import { useRef, useCallback } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { Commit, CommitEdit } from "../types";
import DateField from "./DateField";

export type EditableField = keyof Omit<CommitEdit, "hash">;

interface Props {
  commits: Commit[];
  edits: Record<string, CommitEdit>;
  expanded: string | null;
  onToggle: (hash: string) => void;
  onField: (hash: string, field: EditableField, value: string, original: string) => void;
  onResetRow: (hash: string) => void;
}

/** Reconstruct the full message the way it is stored (subject + blank + body). */
export function fullMessage(c: Commit): string {
  return c.body.trim() ? `${c.subject}\n\n${c.body.trim()}` : c.subject;
}

function shortDate(iso: string): string {
  // Show the stored local time without the offset for compactness.
  return iso.replace("T", " ").replace(/([+-]\d{2}:\d{2})$/, " $1");
}

/** Estimated row heights for the virtualizer. */
const ROW_HEIGHT = 38;
const EDITOR_HEIGHT = 320;

export default function CommitTable({
  commits,
  edits,
  expanded,
  onToggle,
  onField,
  onResetRow,
}: Props) {
  const parentRef = useRef<HTMLDivElement>(null);

  /**
   * Build a flat list of "virtual items" — each commit produces one summary row,
   * plus one editor row if it's expanded. This lets the virtualizer handle
   * variable-height rows without nesting.
   */
  type VRow = { type: "commit"; commit: Commit } | { type: "editor"; commit: Commit };

  const rows: VRow[] = [];
  for (const c of commits) {
    rows.push({ type: "commit", commit: c });
    if (expanded === c.hash) {
      rows.push({ type: "editor", commit: c });
    }
  }

  const estimateSize = useCallback(
    (index: number) => (rows[index].type === "editor" ? EDITOR_HEIGHT : ROW_HEIGHT),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [expanded, commits.length]
  );

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize,
    overscan: 15,
  });

  return (
    <div
      ref={parentRef}
      style={{ overflow: "auto", maxHeight: "calc(100vh - 220px)" }}
    >
      <table className="commits" style={{ width: "100%", borderCollapse: "collapse" }}>
        <thead>
          <tr>
            <th className="col-hash">Commit</th>
            <th className="col-subject">Subject</th>
            <th className="col-author">Author</th>
            <th className="col-date">Author date</th>
            <th className="col-date">Committer date</th>
          </tr>
        </thead>
        <tbody
          style={{
            height: `${virtualizer.getTotalSize()}px`,
            position: "relative",
            display: "block",
          }}
        >
          {virtualizer.getVirtualItems().map((virtualRow) => {
            const row = rows[virtualRow.index];
            const c = row.commit;

            if (row.type === "editor") {
              return (
                <tr
                  key={c.hash + "-editor"}
                  className="editor-row"
                  data-index={virtualRow.index}
                  ref={virtualizer.measureElement}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    transform: `translateY(${virtualRow.start}px)`,
                    display: "table-row",
                  }}
                >
                  <td colSpan={5}>
                    <EditorPane
                      commit={c}
                      edit={edits[c.hash]}
                      onField={onField}
                      onResetRow={onResetRow}
                    />
                  </td>
                </tr>
              );
            }

            // Commit summary row
            const edit = edits[c.hash];
            const dirty = !!edit;
            const eff = (f: EditableField, original: string) =>
              (edit?.[f] as string | undefined) ?? original;
            const origMsg = fullMessage(c);

            return (
              <tr
                key={c.hash}
                className={
                  "commit-row" +
                  (dirty ? " dirty" : "") +
                  (c.isMerge ? " merge" : "") +
                  (expanded === c.hash ? " open" : "")
                }
                onClick={() => onToggle(c.hash)}
                title={c.isMerge ? "Merge commit — editing rebuilds it with both parents" : ""}
                data-index={virtualRow.index}
                ref={virtualizer.measureElement}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  transform: `translateY(${virtualRow.start}px)`,
                  display: "table-row",
                }}
              >
                <td className="col-hash">
                  <code>{c.hash.slice(0, 8)}</code>
                  {c.isMerge && <span className="badge">merge</span>}
                  {c.signed && (
                    <span
                      className="badge signed"
                      title="Signed commit — rewriting removes its signature unless you re-sign"
                    >
                      signed
                    </span>
                  )}
                  {dirty && <span className="dot" title="edited" />}
                </td>
                <td className="col-subject">{eff("message", origMsg).split("\n")[0]}</td>
                <td className="col-author">
                  {eff("authorName", c.authorName)}
                </td>
                <td className="col-date">{shortDate(eff("authorDate", c.authorDate))}</td>
                <td className="col-date">
                  {shortDate(eff("committerDate", c.committerDate))}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/* ── Editor pane (extracted to avoid re-creating in the render loop) ──── */

function EditorPane({
  commit: c,
  edit,
  onField,
  onResetRow,
}: {
  commit: Commit;
  edit: CommitEdit | undefined;
  onField: Props["onField"];
  onResetRow: Props["onResetRow"];
}) {
  const dirty = !!edit;
  const eff = (f: EditableField, original: string) =>
    (edit?.[f] as string | undefined) ?? original;
  const origMsg = fullMessage(c);

  return (
    <div className="editor">
      <label className="full">
        <span>Message</span>
        <textarea
          rows={3}
          value={eff("message", origMsg)}
          onChange={(e) =>
            onField(c.hash, "message", e.target.value, origMsg)
          }
        />
      </label>

      <label>
        <span>Author name</span>
        <input
          value={eff("authorName", c.authorName)}
          onChange={(e) =>
            onField(c.hash, "authorName", e.target.value, c.authorName)
          }
        />
      </label>
      <label>
        <span>Author email</span>
        <input
          value={eff("authorEmail", c.authorEmail)}
          onChange={(e) =>
            onField(c.hash, "authorEmail", e.target.value, c.authorEmail)
          }
        />
      </label>

      <label>
        <span>Author date</span>
        <DateField
          value={eff("authorDate", c.authorDate)}
          edited={edit?.authorDate !== undefined}
          onChange={(v) => onField(c.hash, "authorDate", v, c.authorDate)}
        />
      </label>
      <label>
        <span>Committer date</span>
        <DateField
          value={eff("committerDate", c.committerDate)}
          edited={edit?.committerDate !== undefined}
          onChange={(v) =>
            onField(c.hash, "committerDate", v, c.committerDate)
          }
        />
      </label>

      <label>
        <span>Committer name</span>
        <input
          value={eff("committerName", c.committerName)}
          onChange={(e) =>
            onField(c.hash, "committerName", e.target.value, c.committerName)
          }
        />
      </label>
      <label>
        <span>Committer email</span>
        <input
          value={eff("committerEmail", c.committerEmail)}
          onChange={(e) =>
            onField(c.hash, "committerEmail", e.target.value, c.committerEmail)
          }
        />
      </label>

      <div className="editor-actions">
        <code className="muted">{c.hash}</code>
        {dirty && (
          <button className="link" onClick={() => onResetRow(c.hash)}>
            Reset this commit
          </button>
        )}
      </div>
    </div>
  );
}
