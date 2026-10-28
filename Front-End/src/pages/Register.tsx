import { useState, type FormEvent } from "react";
import { Link, Navigate } from "react-router-dom";
import { ApiError } from "../api";
import { useAuth } from "../auth";

export function RegisterPage() {
  const { user, ready, register } = useAuth();
  const [nome, setNome] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  if (ready && user) {
    return <Navigate to="/" replace />;
  }

  async function onSubmit(event: FormEvent) {
    event.preventDefault();
    setError(null);
    setPending(true);
    try {
      await register(nome, password);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Não foi possível cadastrar.");
    } finally {
      setPending(false);
    }
  }

  return (
    <main className="auth-shell">
      <section className="auth-card">
        <p className="eyebrow">Cofre local</p>
        <h1>Criar cofre</h1>
        <p className="lede">Um nome e uma senha. Os arquivos ficam cifrados só para você.</p>
        <form onSubmit={(e) => void onSubmit(e)}>
          <label htmlFor="nome">Nome</label>
          <input
            id="nome"
            name="nome"
            type="text"
            autoComplete="username"
            value={nome}
            onChange={(e) => setNome(e.target.value)}
            required
            maxLength={50}
          />
          <label htmlFor="password">Senha</label>
          <input
            id="password"
            name="password"
            type="password"
            autoComplete="new-password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            minLength={8}
          />
          <p className="hint">Mínimo de 8 caracteres.</p>
          {error ? <p className="error">{error}</p> : null}
          <button type="submit" disabled={pending}>
            {pending ? "Criando…" : "Cadastrar"}
          </button>
        </form>
        <p className="foot">
          Já tem conta? <Link to="/login">Entrar</Link>
        </p>
      </section>
    </main>
  );
}
