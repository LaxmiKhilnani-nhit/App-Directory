import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import type { SessionUser } from "@app-directory/types";
import { api, setAuthToken } from "./api/client";

const TOKEN_KEY = "appdir.admin.token";

interface AuthContextValue {
  user: SessionUser | null;
  loading: boolean;
  signIn: (email: string, password: string) => Promise<SessionUser>;
  signOut: () => Promise<void>;
  replaceUser: (user: SessionUser) => void;
}

const AuthContext = createContext<AuthContextValue | null>(null);

function readStoredToken(): string | null {
  try {
    return sessionStorage.getItem(TOKEN_KEY);
  } catch {
    return null;
  }
}

function storeToken(token: string | null): void {
  try {
    if (token) sessionStorage.setItem(TOKEN_KEY, token);
    else sessionStorage.removeItem(TOKEN_KEY);
  } catch {
    // Private-mode browsers can refuse storage; the in-memory token still works.
  }
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<SessionUser | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    const token = readStoredToken();

    if (!token) {
      setLoading(false);
      return;
    }

    setAuthToken(token);
    api
      .me()
      .then((sessionUser) => {
        if (!cancelled) setUser(sessionUser);
      })
      .catch(() => {
        if (cancelled) return;
        setAuthToken(null);
        storeToken(null);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const signIn = useCallback(async (email: string, password: string) => {
    const session = await api.login(email, password);
    setAuthToken(session.token);
    storeToken(session.token);
    setUser(session.user);
    return session.user;
  }, []);

  const signOut = useCallback(async () => {
    try {
      await api.logout();
    } catch {
      // The token may already be invalid; clearing locally is what matters.
    }
    setAuthToken(null);
    storeToken(null);
    setUser(null);
  }, []);

  const replaceUser = useCallback((next: SessionUser) => {
    setUser(next);
  }, []);

  const value = useMemo(
    () => ({ user, loading, signIn, signOut, replaceUser }),
    [user, loading, signIn, signOut, replaceUser],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthContextValue {
  const context = useContext(AuthContext);
  if (!context) throw new Error("useAuth must be used inside <AuthProvider>");
  return context;
}
