export interface ArchiveRecord {
  hash: string;
  source: string;
  category: string;
  firstArchivedAt: string;
  lastArchivedAt: string;
  versions: number;
  tags: string[];
}

const STORAGE_KEY = 'crablet-archive-index-v1';

const readAll = (): ArchiveRecord[] => {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
};

const saveAll = (rows: ArchiveRecord[]) => {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(rows));
};

export const archiveIndexService = {
  list: (): ArchiveRecord[] => readAll(),
  upsert: (hash: string, source: string, category: string, tags: string[]) => {
    const rows = readAll();
    const now = new Date().toISOString();
    const idx = rows.findIndex((r) => r.hash === hash);
    if (idx >= 0) {
      rows[idx] = {
        ...rows[idx],
        source,
        category,
        lastArchivedAt: now,
        versions: rows[idx].versions + 1,
        tags: [...new Set([...(rows[idx].tags || []), ...tags])],
      };
    } else {
      rows.push({
        hash,
        source,
        category,
        firstArchivedAt: now,
        lastArchivedAt: now,
        versions: 1,
        tags,
      });
    }
    saveAll(rows);
    return rows[idx >= 0 ? idx : rows.length - 1];
  },
  clear: () => saveAll([]),
};
