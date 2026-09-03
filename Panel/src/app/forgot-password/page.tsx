import Link from "next/link";
import { Mail, Server } from "lucide-react";

export default function ForgotPasswordPage() {
  return (
    <main className="login-page">
      <section className="login-shell" aria-labelledby="forgot-title">
        <div className="brand-mark" aria-hidden="true">
          <Server size={30} strokeWidth={1.8} />
        </div>
        <p className="eyebrow">NT&amp;DBN PANEL</p>
        <h1 id="forgot-title">Forgot password</h1>
        <p className="login-intro">
          Enter the recovery email stored during installation. Mail delivery
          will be wired when SMTP is configured. Operators with root access can
          also reset the bootstrap account on the server.
        </p>

        <form className="login-form" method="post" action="/api/forgot-password">
          <label htmlFor="email">Recovery email</label>
          <div className="input-shell">
            <Mail size={19} aria-hidden="true" />
            <input
              id="email"
              name="email"
              type="email"
              placeholder="you@example.com"
              autoComplete="email"
              required
            />
          </div>
          <button type="submit">Send reset instructions</button>
        </form>

        <Link className="demo-link" href="/">
          Back to sign in
        </Link>
      </section>

      <footer>© 2026 NT&amp;DBN Panel. All rights reserved.</footer>
    </main>
  );
}
