import Link from "next/link";
import { LockKeyhole, Server, UserRound } from "lucide-react";

export default function LoginPage() {
  return (
    <main className="login-page">
      <section className="login-shell" aria-labelledby="login-title">
        <div className="brand-mark" aria-hidden="true">
          <Server size={30} strokeWidth={1.8} />
        </div>
        <p className="eyebrow">NT&amp;DBN PANEL</p>
        <h1 id="login-title">Sign in</h1>
        <p className="login-intro">Access your server workspace.</p>

        {/* method=post avoids putting credentials in the URL (issue #8). */}
        <form className="login-form" method="post" action="/api/login">
          <label htmlFor="username">Username</label>
          <div className="input-shell">
            <UserRound size={19} aria-hidden="true" />
            <input
              id="username"
              name="username"
              type="text"
              placeholder="admin"
              autoComplete="username"
              required
            />
          </div>

          <div className="password-row">
            <label htmlFor="password">Password</label>
            <Link href="/forgot-password">Forgot password?</Link>
          </div>
          <div className="input-shell">
            <LockKeyhole size={19} aria-hidden="true" />
            <input
              id="password"
              name="password"
              type="password"
              placeholder="Enter your password"
              autoComplete="current-password"
              required
            />
          </div>

          <button type="submit">Sign in</button>
        </form>

        <p className="login-intro" style={{ marginTop: 18 }}>
          Full session auth is still in progress. This form always uses POST so
          passwords never appear in the query string. Recovery email is stored
          during install for the forgotten-password entry point.
        </p>

        <Link className="demo-link" href="/dashboard?preview=1">
          View dashboard preview
        </Link>
      </section>

      <footer>© 2026 NT&amp;DBN Panel. All rights reserved.</footer>
    </main>
  );
}
