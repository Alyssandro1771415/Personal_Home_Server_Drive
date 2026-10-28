import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import type { ReactNode } from "react";
import * as api from "./api";
import type { User } from "./api";

type AuthContextValue = {
  user: User | null;
  ready: boolean;
  login: (nome: string, password: string) => Promise<void>;
  register: (nome: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
};

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    async function boot() {
      if (!api.getToken()) {
        setReady(true);
        return;
      }
      try {
        const me = await api.me();
        if (!cancelled) {
          setUser(me);
        }
      } catch {
        if (!cancelled) {
          setUser(null);
        }
      } finally {
        if (!cancelled) {
          setReady(true);
        }
      }
    }
    void boot();
    return () => {
      cancelled = true;
    };
  }, []);

  const login = useCallback(async (nome: string, password: string) => {
    await api.login(nome, password);
    setUser(await api.me());
  }, []);

  const register = useCallback(async (nome: string, password: string) => {
    await api.register(nome, password);
    await login(nome, password);
  }, [login]);

  const logout = useCallback(async () => {
    await api.logout();
    setUser(null);
  }, []);

  const value = useMemo(
    () => ({ user, ready, login, register, logout }),
    [user, ready, login, register, logout],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error("useAuth precisa estar dentro de AuthProvider");
  }
  return ctx;
}
