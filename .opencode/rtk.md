# RTK (Response Token Kiln)

RTK compresses verbose Bash output. Pipe long-running or
output-heavy commands through `rtk compress`:

    kg index --full | rtk compress

Opencode has no native output filter hook, but `rtk`
works as a standard Unix pipe.
