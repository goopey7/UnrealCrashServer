# Unreal Crash Server
Because collecting Unreal crash reports shouldn’t require a SaaS contract.

## Quickstart - Docker
The server is available as a docker image for convenience.

### Example docker-compose.yml
```yml
services:
  unreal-crash-server:
    image: ghcr.io/goopey7/unrealcrashserver:latest
    volumes:
      - ./crashes:/crashes
    environment:
      - RUST_LOG=info # log level
      #- BASE_URL: "https://example.com" # base URL for admin/internal web ui (only required for discord notification links)
      #- CRASH_REPORT_DISCORD: "<discord-webhook-url>" # webhook url for discord notifications
    ports:
      - "8080:8080" # public-facing crash endpoint
      - "8081:8081" # admin/internal web-ui showing crashes
    restart: unless-stopped
```
Then fire up the server with `docker-compose up -d`
The admin UI can be accessed over port 8081 (http://localhost:8081)

> [!WARNING]
> The server is intended to be run behind a secure reverse proxy if exposed publicly

## Required Unreal Engine Configuration
In order to configure your Unreal project to send crash reports you must configure the following:

### Config/DefaultEngine.ini
```ini
[CrashReportClient]
bAgreeToCrashUpload=true
bSendUnattendedBugReports=true
CompanyName="<company-name>"
DataRouterUrl="<url-for-crash-endpoint>" # Ex: http://localhost:8080
UserCommentSizeLimit=4000
bAllowToBeContacted=true
bSendLogFile=true
```
### Config/DefaultGame.ini
```ini
[/Script/UnrealEd.ProjectPackagingSettings]
IncludeCrashReporter=True
```
## Security Considerations
**There is no built-in authentication!** So don't dirctly expose the admin UI (port 8081)!
It's recommended to put the server behind a reverse-proxy such as Cloudflare Tunnels, NGINX, or Traefik and delegate any of your security needs to them.

## Contributing
Contributions are very welcome!
- Make use of GitHub Issues for bug reports and feature requests
- PRs should include a clear description and have minimal unrelated changes
