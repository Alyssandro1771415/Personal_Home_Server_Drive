import { afterEach, describe, expect, it } from "vitest";
import type { DriveFile, DriveFolder } from "./api";
import {
  defaultDir,
  loadSort,
  saveSort,
  sortFiles,
  sortFolders,
} from "./sortListing";

const memory = new Map<string, string>();

function file(partial: Partial<DriveFile> & Pick<DriveFile, "id" | "nome">): DriveFile {
  return {
    mime: null,
    tamanho_bytes: 0,
    folder_id: null,
    ...partial,
  };
}

function folder(partial: Partial<DriveFolder> & Pick<DriveFolder, "id" | "nome">): DriveFolder {
  return {
    parent_id: null,
    ...partial,
  };
}

afterEach(() => {
  memory.clear();
});

function installStorage() {
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    value: {
      getItem: (key: string) => memory.get(key) ?? null,
      setItem: (key: string, value: string) => {
        memory.set(key, value);
      },
      removeItem: (key: string) => {
        memory.delete(key);
      },
    },
  });
}

describe("ordenação do drive", () => {
  it("ordena arquivos por nome com números naturais", () => {
    const sorted = sortFiles(
      [
        file({ id: 1, nome: "foto10.jpg" }),
        file({ id: 2, nome: "foto2.jpg" }),
      ],
      "name",
      "asc",
    );
    expect(sorted.map((item) => item.nome)).toEqual(["foto2.jpg", "foto10.jpg"]);
  });

  it("coloca o maior arquivo primeiro", () => {
    const sorted = sortFiles(
      [
        file({ id: 1, nome: "a", tamanho_bytes: 10 }),
        file({ id: 2, nome: "b", tamanho_bytes: 50 }),
      ],
      "size",
      "desc",
    );
    expect(sorted.map((item) => item.id)).toEqual([2, 1]);
  });

  it("ordena pela data de criação e usa o nome no empate", () => {
    const sorted = sortFiles(
      [
        file({ id: 1, nome: "b", created_at: "2026-01-02T00:00:00Z" }),
        file({ id: 2, nome: "a", created_at: "2026-01-02T00:00:00Z" }),
        file({ id: 3, nome: "c", created_at: "2026-01-01T00:00:00Z" }),
      ],
      "created",
      "asc",
    );
    expect(sorted.map((item) => item.id)).toEqual([3, 2, 1]);
  });

  it("mantém pastas em ordem alfabética quando o critério é tamanho", () => {
    const sorted = sortFolders(
      [folder({ id: 2, nome: "zeta" }), folder({ id: 1, nome: "alfa" })],
      "size",
      "desc",
    );
    expect(sorted.map((item) => item.nome)).toEqual(["alfa", "zeta"]);
  });

  it("guarda e relê a preferência", () => {
    installStorage();
    expect(defaultDir("modified")).toBe("desc");
    saveSort({ key: "type", dir: "asc" });
    expect(loadSort()).toEqual({ key: "type", dir: "asc" });
    memory.set("home-drive-sort", "{");
    expect(loadSort()).toEqual({ key: "name", dir: "asc" });
  });
});
