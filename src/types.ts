/** The repository (and branch) given on the command line. */
export interface InitialRepo {
  path: string;
  branch: string | null;
}

export interface RepoInfo {
  path: string;
  branch: string;
  head: string;
  upstream: string | null;
  aheadOfUpstream: number | null;
}

export interface Branch {
  name: string;
  isHead: boolean;
  head: string;
  upstream: string | null;
  aheadOfUpstream: number | null;
}

export interface Commit {
  hash: string;
  parents: string[];
  authorName: string;
  authorEmail: string;
  authorDate: string; // ISO-8601 with offset, e.g. 2020-01-02T12:00:00+00:00
  committerName: string;
  committerEmail: string;
  committerDate: string;
  subject: string;
  body: string;
  isMerge: boolean;
  signed: boolean;
}

/** Overrides for a single commit. Fields left undefined are unchanged. */
export interface CommitEdit {
  hash: string;
  authorName?: string;
  authorEmail?: string;
  authorDate?: string;
  committerName?: string;
  committerEmail?: string;
  committerDate?: string;
  message?: string; // full message (subject + body)
}

export interface FieldChange {
  hash: string;
  field: string;
  oldValue: string;
  newValue: string;
}

export interface ApplyResult {
  newHead: string;
  backupRef: string;
  rewrittenCount: number;
}

export interface Backup {
  refName: string;
  target: string;
  createdAt: string; // unix epoch seconds, as string
  branch: string;
  subject: string;
}
