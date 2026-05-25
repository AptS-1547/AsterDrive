# Documentation Contribution Guide

This page is for people preparing to change AsterDrive documentation. We want every page to help readers complete one clear task, so before adding content, first confirm which reading path it belongs to.

## Decide Where It Belongs First

AsterDrive documentation is layered by reader task:

| What you are writing | Where it goes | Examples |
| --- | --- | --- |
| First use, daily operations, administrator workflows | `guide/` | User manual, common workflows, follower nodes, file editing |
| Startup configuration, backend system settings, storage policy descriptions | `config/` | Server, database, system settings, storage policies |
| Deployment, launch, upgrade, backup, troubleshooting | `deployment/` | Docker, systemd, reverse proxy, troubleshooting |
| Concept explanations, indexes, problem routing | Reference pages under `guide/` | Glossary, FAQ triage, error codes |

When unsure, ask first: **what task did the reader open this page to complete?**

- "I want to use this feature" -> `guide/`
- "I need to change which configuration" -> `config/`
- "I need to keep the service running steadily" -> `deployment/`
- "I do not understand a term / do not know where to look" -> glossary or FAQ

## Be Careful with the Top Nav

The top nav only handles broad direction jumps:

- Start
- Use
- Manage
- Operate
- Versions

Prefer adding new pages into the fixed sidebar reading flow. Only consider changing the top nav when a new first-level reader task appears.

## The Sidebar Is a Reading Flow

The sidebar is fixed across the site and does not switch by directory. Its goal is to keep readers aware of the whole documentation structure.

Default order:

1. Start
2. Daily Use
3. Management and Configuration
4. Deployment and Operations
5. Reference and Project

When adding a page, insert it where readers first need it. Do not sort by filename.

## Terminology Should Match the UI

Prefer using the product UI wording in documentation. When needed, add an English or internal name on first mention.

Recommended wording:

- `Follower Nodes`, and explain that they are followers when needed
- `Primary node`, and add `primary` when needed
- `Follower node`, and add `follower` when needed
- `Ingress target`
- `Storage policy`
- `Policy group`
- `System settings`
- `Public site URL`
- `Preview app`
- `Audit log`

Avoid mixing multiple names on the same page, such as calling something "follower node", then "follower instance", then "remote storage instance". Explain it clearly once, then keep the same name.

## Help Readers Orient at the Start

Long pages should ideally start with three things:

- What the page covers
- When to read it
- Where to operate, or which quick-reference table to read first

Recommended structure:

```md
# Page Title

::: tip What this page covers
One sentence defining the boundary. Avoid repeating large parts of adjacent pages here.
:::

## Entry Quick Reference

| What you want to do | Where to go |
| --- | --- |
| ... | ... |
```

## Link Rules

Prefer absolute paths for site links:

```md
[System Settings](/en/config/runtime)
[Follower Nodes](/en/guide/remote-nodes)
[Troubleshooting](/en/deployment/troubleshooting)
```

Same-directory short links are also fine, but avoid relative paths such as `../guide/...` across directories. Absolute paths are easier to read and more stable when files move later.

## Writing Rules

- Give the conclusion first, then details
- Use tables for quick reference and lists for steps
- Use backticks for configuration items, paths, and commands
- Use `warning` for dangerous operations
- Use `details` for optional background knowledge
- Do not write promises for features that have not been merged
- Do not copy large sections from another page just to be "complete"; link to that page instead

## Flow Diagram Rules

When a plain-text flow diagram is enough, prefer a `text` code block:

```text
User action
  |
  +-- System decision
      |
      +-- Result A
      +-- Result B
```

Usually there is no need to introduce an extra plugin just for diagrams. Documentation site stability and maintainability matter more.

## Verify After Changes

After changing documentation, run at least:

```bash
bun run docs:build
```

If you changed navigation, logo, sidebar, or the homepage, it is better to also run:

```bash
bun run docs:dev
```

Then click through:

- Homepage entry points
- Top nav dropdowns
- Fixed sidebar collapse
- New pages
- Edit-this-page links
- Dark / light logos

Successful build is only the baseline. You still need to preview it yourself and confirm readers can follow the entry points and sidebar to find the content.
