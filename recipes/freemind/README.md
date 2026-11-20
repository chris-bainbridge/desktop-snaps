# freemind

The recipe builds FreeMind from the SourceForge git repository at the commit
pinned by `source-commit` in `snapcraft.yaml`, rather than from an upstream
release archive, so a clean checkout needs no manual download.

Printing is disabled: Java printing does not work under strict confinement.

OpenJDK 21 is staged deliberately. FreeMind 1.1.0 installs a `SecurityManager`
at startup, which Java 18+ only permits with
`-Djava.security.manager=allow`; Java 24 and later (JEP 486) removed the API
entirely, so the runtime cannot be bumped further.
