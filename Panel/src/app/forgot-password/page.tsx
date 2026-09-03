import Link from "next/link";
import { Mail, Server } from "lucide-react";

export default function ForgotPasswordPage() {
  return (
    <main className="login-page">
      <section className="login-shell" aria-labelledby="forgot-title">
        <div className="brand-mark" aria-hidden="true">
          <Server size={30} strokeWidth={1.8} />
        </div>
        <p className="eyebrow">CPN PANEL</p>
        <h1 id="forgot-title">Forgot password</h1>
        <p className="login-intro">
          Enter your username or email. If a matching account exists, a reset
          message will be sent when mail delivery is configured.
        </p>

        <form className="login-form" method="post" action="/api/forgot-password">
          <label htmlFor="account">Username/Email</label>
          <div className="input-shell">
            <Mail size={19} aria-hidden="true" />
            <input
              id="account"
              name="account"
              type="text"
              placeholder="Admin or you@example.com"
              autoComplete="username"
              required
            />
          </div>
          <button type="submit">Send reset instructions</button>
        </form>

        <Link className="demo-link" href="/">
          Back to sign in
        </Link>
      </section>

      <footer>© 2026 CPN Panel. All rights reserved.</footer>
    </main>
  );
}
