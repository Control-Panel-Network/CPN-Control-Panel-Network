import Link from "next/link";
import { LockKeyhole, Mail, Server } from "lucide-react";

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

        <form className="login-form" action="/dashboard">
          <label htmlFor="email">Email address</label>
          <div className="input-shell">
            <Mail size={19} aria-hidden="true" />
            <input id="email" name="email" type="email" placeholder="you@example.com" required />
          </div>

          <div className="password-row">
            <label htmlFor="password">Password</label>
            <a href="#">Forgot password?</a>
          </div>
          <div className="input-shell">
            <LockKeyhole size={19} aria-hidden="true" />
            <input id="password" name="password" type="password" placeholder="Enter your password" required />
          </div>

          <button type="submit">Sign in</button>
        </form>

        <Link className="demo-link" href="/dashboard">
          View dashboard preview
        </Link>
      </section>

      <footer>© 2026 NT&amp;DBN Panel. All rights reserved.</footer>
    </main>
  );
}
