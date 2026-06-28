export const KGXVerifyFinished = async ({ directory, worktree, client }) => {
  let running = false
  const root = worktree || directory
  const script = `${root}/.kgx/hooks/verify-finished.sh`

  return {
    event: async ({ event }) => {
      if (event.type !== "session.idle" || running) return
      running = true
      try {
        const proc = Bun.spawn(["sh", script, "--plain"], {
          cwd: root,
          stdout: "pipe",
          stderr: "pipe",
        })
        const [stdout, stderr, exitCode] = await Promise.all([
          new Response(proc.stdout).text(),
          new Response(proc.stderr).text(),
          proc.exited,
        ])
        if (exitCode !== 0) {
          await client.app.log({
            body: {
              service: "kgx-verify-finished",
              level: "error",
              message: stderr || stdout || "Finish verification failed",
            },
          })
          throw new Error(stderr || stdout || "Finish verification failed")
        }
      } finally {
        running = false
      }
    },
  }
}
