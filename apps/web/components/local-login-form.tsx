export function LocalLoginForm() {
  return (
    <section className="developer-shell">
      <h1>Sign in to manage API keys and crawler workers</h1>
      <p>
        Local auth is enabled for this deployment. Use the configured username and
        password to enter the developer portal.
      </p>
      <form action="/api/auth/local-login" method="post" className="local-login-form">
        <input name="username" placeholder="Username" autoComplete="username" />
        <input
          name="password"
          type="password"
          placeholder="Password"
          autoComplete="current-password"
        />
        <button type="submit">Sign in</button>
      </form>
    </section>
  );
}
