import { DeveloperPortal } from "@/components/developer-portal";
import { LocalLoginForm } from "@/components/local-login-form";
import { loadCrawlOverview, loadDeveloperUsage } from "@/lib/api";
import { getSession, isLocalAuthConfigured } from "@/lib/session";

export default async function DevelopersPage() {
  const session = await getSession();
  const localAuthConfigured = isLocalAuthConfigured();

  return (
    <main className="page-shell">
      <header className="site-header">
        <div className="brand-lockup">
          <div className="brand-mark">FV</div>
          <div className="brand-copy">
            <strong>FindVerse</strong>
            <span>Developer portal</span>
          </div>
        </div>
        <nav className="top-nav">
          <a href="/">Home</a>
          <a href="/search?q=search+api">Search</a>
          <a href="/docs">Docs</a>
          {session ? <a href="/api/auth/signout">Sign out</a> : null}
        </nav>
      </header>

      {!localAuthConfigured ? (
        <section className="developer-shell">
          <h1>Local auth is not configured</h1>
          <p>
            Set <code>AUTH_SECRET</code>, <code>FINDVERSE_LOCAL_ADMIN_USERNAME</code>,
            and <code>FINDVERSE_LOCAL_ADMIN_PASSWORD</code> to enable the developer
            login flow.
          </p>
        </section>
      ) : !session ? (
        <LocalLoginForm />
      ) : (
        <DeveloperPortal
          session={session}
          initialUsage={await loadDeveloperUsage(session.id)}
          initialOverview={await loadCrawlOverview(session.id)}
        />
      )}
    </main>
  );
}
