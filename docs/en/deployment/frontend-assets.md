# Upgrades and Browser Cache

::: tip What this page covers
AsterDrive's pages and server are served by **the same program**. A normal upgrade means upgrading the same image or the same binary; you do not need to deploy static assets separately.
If you see a half-updated state where "the page is still old but the service is already new", skip to [If You Customized Packaging or Replaced Assets](#if-you-customized-packaging-or-replaced-assets).
:::

AsterDrive's browser page, public sharing page, and server are served by the same program.  
In normal deployments, upgrading the same image or binary updates the pages and server together.

## Recommended Approach

- Docker deployment: upgrade directly to the new image version.
- systemd or single-binary deployment: replace the `aster_drive` binary directly.
- After upgrading, refresh the browser page and test login, upload, sharing, WebDAV, and any external opener you are using.

## When Version Mismatch Happens

If you do not use the official image or binary of the same version directly, and instead manually mix page assets and server from different versions, you may see a half-updated state where "the page is still old but the service is already new".

Common symptoms:

- The page opens, but buttons fail.
- New-version feature entries do not appear on the page.
- Some dialogs open, but submission fails.
- WOPI or external preview entries display incorrectly, or opening them behaves inconsistently with the backend version.

## If You Customized Packaging or Replaced Assets

If you did not use a ready-made image or binary directly, and instead built or replaced page assets yourself, make sure page assets and server come from the same version during upgrade.

Recommended sequence:

1. Back up `data/config.toml`, the database, and upload directories.
2. Stop the old service.
3. Replace the new server and new page assets in one operation.
4. Start the service.
5. Refresh the browser cache and run a full acceptance check.

## Browser Pages Do Not Need Separate Deployment

The browser page, public sharing page, and static assets are all returned by the same AsterDrive service.  
The reverse proxy usually only needs to proxy the whole site to AsterDrive. You do not need to deploy a separate static asset service.

## If the Page Looks Wrong After Upgrade

1. Confirm the service has upgraded to the expected version.
2. Force-refresh the browser page.
3. If you use Docker, confirm the container is using the latest image.
4. If you replaced page assets yourself, confirm the page and server come from the same version.
