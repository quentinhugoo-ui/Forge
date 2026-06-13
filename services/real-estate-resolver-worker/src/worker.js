function json(body, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      "cache-control": "no-store",
    },
  });
}

function readBearer(headers) {
  const auth = headers.get("authorization") || "";
  const parts = auth.split(" ");
  if (parts.length === 2 && /^bearer$/i.test(parts[0])) return parts[1].trim();
  return "";
}

function normalizeText(value) {
  return String(value || "").trim();
}

function normalizeForMatch(value) {
  return normalizeText(value)
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

function tokenize(value) {
  const normalized = normalizeForMatch(value);
  if (!normalized) return [];
  return normalized.split(" ").filter((item) => item.length >= 2);
}

function pickString(obj, keys) {
  for (const key of keys) {
    const value = obj?.[key];
    if (typeof value === "string" && value.trim()) return value.trim();
  }
  return "";
}

function pickNumber(obj, keys) {
  for (const key of keys) {
    const value = obj?.[key];
    if (typeof value === "number" && Number.isFinite(value)) return value;
  }
  return null;
}

function pickArray(obj, keys) {
  for (const key of keys) {
    const value = obj?.[key];
    if (Array.isArray(value)) return value;
  }
  return [];
}

function mapAddressComponents(components) {
  if (!Array.isArray(components)) return [];
  return components
    .map((item) => ({
      longText: pickString(item, ["longText", "long_name", "name"]),
      shortText: pickString(item, ["shortText", "short_name"]),
      types: Array.isArray(item?.types) ? item.types : [],
      languageCode: pickString(item, ["languageCode", "language_code"]),
    }))
    .filter((item) => item.longText || item.shortText || item.types.length);
}

function mapOpeningHours(openingHours) {
  if (!openingHours || typeof openingHours !== "object") return null;
  return {
    openNow: openingHours?.openNow === true,
    weekdayDescriptions: Array.isArray(openingHours?.weekdayDescriptions)
      ? openingHours.weekdayDescriptions
      : [],
    periods: Array.isArray(openingHours?.periods) ? openingHours.periods : [],
    nextOpenTime: pickString(openingHours, ["nextOpenTime"]),
    nextCloseTime: pickString(openingHours, ["nextCloseTime"]),
  };
}

function mapViewport(viewport) {
  if (!viewport || typeof viewport !== "object") return null;
  return {
    low: viewport?.low || null,
    high: viewport?.high || null,
  };
}

function mapPlace(place) {
  const displayName = pickString(place?.displayName, ["text"]) || pickString(place, ["displayName"]);
  const formattedAddress = pickString(place, ["formattedAddress", "shortFormattedAddress", "adrFormatAddress"]);
  const websiteUri = pickString(place, ["websiteUri"]);
  const googleMapsUri = pickString(place, ["googleMapsUri"]);
  const nationalPhoneNumber = pickString(place, ["nationalPhoneNumber", "internationalPhoneNumber"]);
  const internationalPhoneNumber = pickString(place, ["internationalPhoneNumber"]);
  const lat = pickNumber(place?.location, ["latitude", "lat"]);
  const lng = pickNumber(place?.location, ["longitude", "lng"]);
  const location = lat != null && lng != null ? { lat, lng } : null;
  const addressComponents = mapAddressComponents(place?.addressComponents);
  const openingHours = mapOpeningHours(place?.regularOpeningHours);
  const currentOpeningHours = mapOpeningHours(place?.currentOpeningHours);
  const viewport = mapViewport(place?.viewport);

  const extended = {
    placeResourceName: pickString(place, ["name"]),
    id: pickString(place, ["id"]),
    displayName,
    primaryType: pickString(place, ["primaryType"]),
    primaryTypeDisplayName:
      pickString(place?.primaryTypeDisplayName, ["text"]) || pickString(place, ["primaryTypeDisplayName"]),
    types: pickArray(place, ["types"]),
    formattedAddress,
    shortFormattedAddress: pickString(place, ["shortFormattedAddress"]),
    adrFormatAddress: pickString(place, ["adrFormatAddress"]),
    location,
    viewport,
    plusCode: place?.plusCode || null,
    utcOffsetMinutes: pickNumber(place, ["utcOffsetMinutes"]),
    businessStatus: pickString(place, ["businessStatus"]),
    rating: pickNumber(place, ["rating"]),
    userRatingCount: pickNumber(place, ["userRatingCount"]),
    googleMapsUri,
    websiteUri,
    nationalPhoneNumber,
    internationalPhoneNumber,
    regularOpeningHours: openingHours,
    currentOpeningHours,
    addressComponents,
    iconMaskBaseUri: pickString(place, ["iconMaskBaseUri"]),
    iconBackgroundColor: pickString(place, ["iconBackgroundColor"]),
    pureServiceAreaBusiness: place?.pureServiceAreaBusiness === true,
    takeout: place?.takeout === true,
    delivery: place?.delivery === true,
    dineIn: place?.dineIn === true,
    curbsidePickup: place?.curbsidePickup === true,
    reservable: place?.reservable === true,
    servesBreakfast: place?.servesBreakfast === true,
    servesLunch: place?.servesLunch === true,
    servesDinner: place?.servesDinner === true,
    servesBeer: place?.servesBeer === true,
    servesWine: place?.servesWine === true,
    paymentOptions: place?.paymentOptions || null,
    parkingOptions: place?.parkingOptions || null,
    accessibilityOptions: place?.accessibilityOptions || null,
    editorialSummary:
      pickString(place?.editorialSummary, ["text"]) || pickString(place, ["editorialSummary"]),
  };

  return {
    displayName,
    formattedAddress,
    websiteUri,
    googleMapsUri,
    nationalPhoneNumber,
    location,
    agencyExtended: extended,
  };
}

function placeContactCompletenessScore(place) {
  if (!place || typeof place !== "object") return 0;
  let score = 0;
  if (normalizeText(place.formattedAddress)) score += 1;
  if (normalizeText(place.nationalPhoneNumber)) score += 1;
  if (normalizeText(place.websiteUri)) score += 1;
  if (place.location && Number.isFinite(place.location.lat) && Number.isFinite(place.location.lng)) score += 1;
  return score;
}

function mergePlaceObjects(basePlace, detailsPlace) {
  if (!detailsPlace || typeof detailsPlace !== "object") return basePlace;
  if (!basePlace || typeof basePlace !== "object") return detailsPlace;
  return {
    ...basePlace,
    ...detailsPlace,
    displayName: detailsPlace.displayName || basePlace.displayName,
    location: detailsPlace.location || basePlace.location,
    viewport: detailsPlace.viewport || basePlace.viewport,
    addressComponents: Array.isArray(detailsPlace.addressComponents) && detailsPlace.addressComponents.length
      ? detailsPlace.addressComponents
      : basePlace.addressComponents,
    regularOpeningHours: detailsPlace.regularOpeningHours || basePlace.regularOpeningHours,
    currentOpeningHours: detailsPlace.currentOpeningHours || basePlace.currentOpeningHours,
    types: Array.isArray(detailsPlace.types) && detailsPlace.types.length ? detailsPlace.types : basePlace.types,
  };
}

async function fetchPlaceDetails(apiKey, placeResourceName, payload) {
  const resource = normalizeText(placeResourceName);
  if (!resource) return null;
  const languageCode = normalizeText(payload?.languageCode || "fr");
  const regionCode = normalizeText(payload?.countryCode || "FR");
  const url = `https://places.googleapis.com/v1/${resource}?languageCode=${encodeURIComponent(languageCode)}&regionCode=${encodeURIComponent(regionCode)}`;
  const upstream = await fetch(url, {
    method: "GET",
    headers: {
      "x-goog-api-key": apiKey,
      "x-goog-fieldmask":
        "name,id,displayName,primaryType,primaryTypeDisplayName,types,formattedAddress,shortFormattedAddress,adrFormatAddress,addressComponents,location,viewport,plusCode,utcOffsetMinutes,businessStatus,rating,userRatingCount,googleMapsUri,websiteUri,nationalPhoneNumber,internationalPhoneNumber,regularOpeningHours,currentOpeningHours,iconMaskBaseUri,iconBackgroundColor,pureServiceAreaBusiness,takeout,delivery,dineIn,curbsidePickup,reservable,servesBreakfast,servesLunch,servesDinner,servesBeer,servesWine,paymentOptions,parkingOptions,accessibilityOptions,editorialSummary",
    },
  });
  if (!upstream.ok) return null;
  const text = await upstream.text();
  try {
    const parsed = text ? JSON.parse(text) : {};
    if (parsed && typeof parsed === "object") return parsed;
  } catch (_) {}
  return null;
}

function scorePlace(place, payload) {
  const placeName = pickString(place?.displayName, ["text"]) || pickString(place, ["displayName"]);
  const address = pickString(place, ["formattedAddress", "shortFormattedAddress", "adrFormatAddress"]);
  const placeNameNorm = normalizeForMatch(placeName);
  const addressNorm = normalizeForMatch(address);

  const requestedAgency = normalizeText(payload?.agencyName) || normalizeText(payload?.name);
  const requestedCity = normalizeText(payload?.city);
  const requestedAgencyNorm = normalizeForMatch(requestedAgency);
  const requestedCityNorm = normalizeForMatch(requestedCity);
  const requestedAgencyTokens = tokenize(requestedAgency);

  let score = 0;
  if (requestedAgencyNorm && placeNameNorm === requestedAgencyNorm) score += 120;
  if (requestedAgencyNorm && placeNameNorm.includes(requestedAgencyNorm)) score += 80;

  if (requestedAgencyTokens.length) {
    const matched = requestedAgencyTokens.filter((token) => placeNameNorm.includes(token)).length;
    score += Math.round((matched / requestedAgencyTokens.length) * 70);
  }

  if (requestedCityNorm && (addressNorm.includes(requestedCityNorm) || placeNameNorm.includes(requestedCityNorm))) {
    score += 35;
  }

  const types = pickArray(place, ["types"]);
  if (types.includes("real_estate_agency")) score += 10;

  return score;
}

async function searchGooglePlaces(apiKey, payload) {
  const query =
    normalizeText(payload?.query) ||
    `${normalizeText(payload?.agencyName) || normalizeText(payload?.name)} ${normalizeText(payload?.city)}`.trim();
  if (!query) return null;

  const upstream = await fetch("https://places.googleapis.com/v1/places:searchText", {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-goog-api-key": apiKey,
      "x-goog-fieldmask":
        "places.name,places.id,places.displayName,places.primaryType,places.primaryTypeDisplayName,places.types,places.formattedAddress,places.shortFormattedAddress,places.adrFormatAddress,places.addressComponents,places.location,places.viewport,places.plusCode,places.utcOffsetMinutes,places.businessStatus,places.rating,places.userRatingCount,places.googleMapsUri,places.websiteUri,places.nationalPhoneNumber,places.internationalPhoneNumber,places.regularOpeningHours,places.currentOpeningHours,places.iconMaskBaseUri,places.iconBackgroundColor,places.pureServiceAreaBusiness,places.takeout,places.delivery,places.dineIn,places.curbsidePickup,places.reservable,places.servesBreakfast,places.servesLunch,places.servesDinner,places.servesBeer,places.servesWine,places.paymentOptions,places.parkingOptions,places.accessibilityOptions,places.editorialSummary",
    },
    body: JSON.stringify({
      textQuery: query,
      languageCode: normalizeText(payload?.languageCode || "fr"),
      regionCode: normalizeText(payload?.countryCode || "FR"),
      maxResultCount: Math.max(1, Math.min(Number(payload?.maxResultCount || 5), 10)),
    }),
  });

  const text = await upstream.text();
  let parsed = {};
  try {
    parsed = text ? JSON.parse(text) : {};
  } catch (_) {
    parsed = {};
  }

  if (!upstream.ok) {
    return {
      error: `google_places_status_${upstream.status}`,
      details: text.slice(0, 400),
      place: null,
    };
  }

  const places = Array.isArray(parsed?.places) ? parsed.places : [];
  if (!places.length) return { error: "no_match", details: "", place: null };

  const ranked = places
    .map((item) => ({ item, score: scorePlace(item, payload) }))
    .sort((a, b) => b.score - a.score);
  const topCandidates = ranked.slice(0, Math.min(ranked.length, 5));
  let bestPlace = null;
  let bestScore = -1;
  for (const candidate of topCandidates) {
    let selected = candidate.item;
    const resourceName = pickString(candidate.item, ["name"]);
    const preMapped = mapPlace(selected);
    const requiresDetails = placeContactCompletenessScore(preMapped) < 3;
    if (requiresDetails && resourceName) {
      const detailed = await fetchPlaceDetails(apiKey, resourceName, payload);
      if (detailed) selected = mergePlaceObjects(candidate.item, detailed);
    }
    const mapped = mapPlace(selected);
    const quality = (candidate.score * 10) + placeContactCompletenessScore(mapped);
    if (quality > bestScore) {
      bestScore = quality;
      bestPlace = mapped;
    }
  }
  if (!bestPlace) return { error: "no_match", details: "", place: null };
  return { error: "", details: "", place: bestPlace };
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname === "/health") {
      return json({ ok: true, service: "forge-real-estate-resolver-worker" });
    }

    if (url.pathname !== "/api/agency/resolve") {
      return json({ error: "not_found" }, 404);
    }

    if (request.method !== "POST") {
      return json({ error: "method_not_allowed" }, 405);
    }

    const expectedToken = normalizeText(env.FORGE_REAL_ESTATE_BACKEND_TOKEN || env.FORGE_TOKEN);
    if (expectedToken) {
      const actualToken = readBearer(request.headers);
      if (!actualToken || actualToken !== expectedToken) {
        return json({ error: "unauthorized" }, 401);
      }
    }

    const apiKey = normalizeText(env.GOOGLE_API_KEY);
    if (!apiKey) return json({ error: "missing_google_api_key" }, 500);

    let payload = {};
    try {
      payload = await request.json();
    } catch (_) {
      return json({ error: "invalid_json" }, 400);
    }

    const result = await searchGooglePlaces(apiKey, payload);
    if (!result || !result.place) {
      return json({
        agency: null,
        meta: {
          resolverSource: "google-places",
          scope: normalizeText(payload?.scope || "real-estate-onboarding"),
          surface: normalizeText(payload?.surface || "forge-ui"),
          countryCode: normalizeText(payload?.countryCode || "FR"),
          echoQuery: normalizeText(payload?.query),
          error: result?.error || "no_match",
        },
      });
    }

    return json({
      agency: {
        confidence: 0.99,
        ...result.place,
        source: "google-places",
      },
      agencyExtended: result.place?.agencyExtended || null,
      meta: {
        resolverSource: "google-places",
        scope: normalizeText(payload?.scope || "real-estate-onboarding"),
        surface: normalizeText(payload?.surface || "forge-ui"),
        countryCode: normalizeText(payload?.countryCode || "FR"),
        echoQuery: normalizeText(payload?.query),
      },
    });
  },
};
