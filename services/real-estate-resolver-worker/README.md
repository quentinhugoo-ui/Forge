# Forge Real Estate Resolver (Cloudflare Worker)

## Deploy rapide

1. Installer Wrangler:
`npm i -g wrangler`

2. Se connecter:
`wrangler login`

3. Aller dans ce dossier:
`cd services/real-estate-resolver-worker`

4. Ajouter les secrets:
`wrangler secret put GOOGLE_API_KEY`
`wrangler secret put FORGE_REAL_ESTATE_BACKEND_TOKEN`

5. Deployer:
`wrangler deploy`

## Endpoints

- `GET /health`
- `POST /api/agency/resolve`

Exemple de body:

```json
{
  "agencyName": "Agence Valerie Duparque",
  "city": "Marcq en Baroeul",
  "query": "Agence Valerie Duparque Marcq en Baroeul agence immobiliere",
  "countryCode": "FR",
  "surface": "forge-ui",
  "scope": "real-estate-onboarding",
  "languageCode": "fr",
  "maxResultCount": 3
}
```

## Donnees renvoyees

- `agency` (compatible onboarding Forge):
`displayName`, `formattedAddress`, `websiteUri`, `googleMapsUri`, `nationalPhoneNumber`, `location.lat/lng`
- `agencyExtended` (payload riche pour usages hors onboarding):
`id`, `name`, `types`, `businessStatus`, `rating`, `userRatingCount`, `addressComponents`, `regularOpeningHours`, `currentOpeningHours`, `utcOffsetMinutes`, `viewport`, `plusCode`, `internationalPhoneNumber`, `paymentOptions`, `parkingOptions`, `accessibilityOptions`, etc.

## Rebrancher Forge

Dans `C:\\Users\\quent\\.forge\\real-estate.env`:

```env
FORGE_REAL_ESTATE_BACKEND_URL="https://<ton-worker>.workers.dev"
FORGE_REAL_ESTATE_BACKEND_TOKEN="<ton-token>"
```
