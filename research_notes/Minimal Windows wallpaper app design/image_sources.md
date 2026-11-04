# Image sources and APIs for backreel (state as of 4 November 2026)

**How to read these notes.**

- "Live test" means I made the request on 2026-11-04 with the generic client name `backreel-research/0.1` and no personal identity. Live-test links point at the endpoint itself.
- "Docs" means I read the provider's page, but through a fetch tool that extracts text with a small model. Treat those quotations as close paraphrase and check them against the linked page before any legal reliance.
- Four pages were read as raw text and grepped, so their quotes are verbatim: the Microsoft Services Agreement, the Rijksmuseum data policy, the NASA API catalog and the Cloudflare Workers pricing page.
- "Snippet" means I saw only search-result text, because the page itself refused the fetch.
- No API key was used for Unsplash, Pexels, Pixabay or Flickr, so their behaviour below is documented, not tested.

## For each source, what are the endpoint, authentication, rate limits, maximum resolution, category support, attribution requirements, and terms on caching, hotlinking and wallpaper use?

### Takeaway
Ten sources work today with no registered key, but only the open-licence ones (Wikimedia, NASA, the four museums) have clear reuse terms. Three things have changed recently and would break code written from older guides: the legacy APOD API is already returning wrong data and goes offline on 1 December 2026, the Met's v1 search was retired on 1 October 2026, and the old Rijksmuseum API now returns 410.

### Cited Findings

#### Summary table

| Source | Key needed | Rate limit | Largest image seen or documented | Categories | Status |
|---|---|---|---|---|---|
| Bing HPImageArchive | No | Not published | 3840x2160 (live) | None | Unofficial, works |
| Windows Spotlight v4 | No | Not published | 3840x2160 (live) | None | Unofficial, works |
| Unsplash | Yes | 50/hr demo, 1000/hr production | Original, resizable | Search, topics, collections | Wallpaper apps barred |
| Pexels | Yes | 200/hr, 20,000/month | Original | Search, collections | Wallpaper apps barred |
| Pixabay | Yes | 100 per 60s | 1280px by default; 1920px and original need approval | 20 fixed categories | Licence restricts wallpaper distribution |
| Wallhaven | No for SFW | 45/min | Original (3840x2160 and larger live) | Search, tags, colours | Works; image rights unclear |
| NASA APOD (new) | No (live) | Not published | About 4144x2822 in one sample | None | Migrate now |
| NASA Image Library | No | Not found | Varies, 2800x1400 in one sample | Keyword search | Works |
| Wikimedia | No | 200/min with a compliant User-Agent | Original (5174x3449 live) | Categories, search | Works |
| Reddit | Yes, plus approval | 100/min per OAuth client | Varies | Subreddits | Closed to new apps without approval |
| Flickr | Yes | 3600/hr per key, all users combined | Original where shared | Tags, text, licence filter | Commercial use needs permission |
| Smithsonian | Yes (api.data.gov) | 1000/hr registered | 4000px long edge (live) | Topic facets | Works |
| The Met | No | 80/second requested | 4000x3184 (live) | Tags, departments | v1.1 search only |
| Art Institute of Chicago | No | 60 (live header) | 3000px wide tested | Subjects, classifications | Images need a courtesy header |
| Rijksmuseum | No | Not published | 5862x5197 (live) | Thin search | New API only |
| National Gallery of Art | No | Not published | 4096px long edge (live) | None via API; CSV data | No search API |
| NatGeo Photo of the Day | n/a | n/a | 3072x2049 (live) | None | Page only, no API |
| Google Earth View | No | Not published | 1800x1200 (live) | Country only | Legacy assets; terms bar it |
| Apple aerials | n/a | n/a | Video, not stills | None | Not relevant |

#### Bing daily image (unofficial)
- **Endpoint:** `https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=8&mkt=en-US` returned 200 JSON with `url`, `urlbase`, `copyright`, `copyrightlink`, `title`, `startdate` and `wp:true`, with no key — [Live test](https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=8&mkt=en-US)
- **Unofficial status:** I found no Microsoft documentation of this endpoint. Rate limits are not published, and no rate-limit headers came back — [Live test](https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=1&mkt=en-US)
- **Sizes:** appending to `urlbase` gave `_UHD.jpg` at 3840x2160 (3.7 MB), `_1920x1200.jpg`, `_1920x1080.jpg` and a portrait `_1080x1920.jpg` — Live test against `https://www.bing.com/th?id=OHR.AlpineBraies_EN-US6636326357_UHD.jpg`
- **History window:** `idx` above 7 is clamped. `idx=7`, `8`, `15`, `16` and `20` all returned the same 8 images, so the reachable window was 15 days (20261020 to 20261103) — Live test
- **Markets:** en-US, en-GB, en-CA, en-IN, de-DE, fr-FR, ja-JP, zh-CN, es-ES, it-IT and pt-BR returned market-tagged images. `en-AU` and the invalid `xx-XX` both fell back to the `ROW` image. Images and rollover times differ by market (`fullstartdate` was 0800 for en-US and 2300 for de-DE; the time zone is not documented) — Live test. A community archive lists the same market set plus CA-fr — [npanuhin/Bing-Wallpaper-Archive](https://github.com/npanuhin/Bing-Wallpaper-Archive)
- **Terms:** "The articles, text, photos, maps, videos, video players, and third-party material available on Bing and MSN, including through Microsoft bots, applications and programs, are for your noncommercial, personal use only" (section 14.f.i). Other uses, including downloading, copying or redistributing, need specific authorisation from Microsoft or the rights holder, or a basis in copyright law — [Microsoft Services Agreement](https://www.microsoft.com/en-us/servicesagreement)
- **Wallpaper statement:** community projects quote Microsoft as saying "Use of this image is restricted to wallpaper only" and state that images belong to "Microsoft, Getty Images, etc." — [npanuhin/Bing-Wallpaper-Archive](https://github.com/npanuhin/Bing-Wallpaper-Archive). I could not find that wording on a Microsoft page. Microsoft's help says most daily images can be downloaded for use as wallpaper — [Microsoft support, snippet](https://support.microsoft.com/en-us/bing/explore-the-homepage)
- **Attribution:** no stated rule, but the `copyright` field carries the credit, such as "© leonid_tit/Getty Images" — Live test

#### Windows Spotlight (unofficial)
- **v4 endpoint:** `https://fd.api.iris.microsoft.com/v4/api/selection?placement=88000820&bcnt=4&country=US&locale=en-US&fmt=json` returned 200 with no key. Each `batchrsp.items[].item` is a JSON string that must be parsed a second time — [Live test](https://fd.api.iris.microsoft.com/v4/api/selection?placement=88000820&bcnt=4&country=US&locale=en-US&fmt=json)
- **Payload:** `ad.landscapeImage.asset` (3840x2160, 2.1 MB, confirmed by download), `ad.portraitImage.asset` (1080x1920), `title`, `description` and `iconHoverText` holding the credit — Live test
- **Batch size:** `bcnt=20` still returned 4 items, matching the community note that the range is 1 to 4 — Live test; [SpotlightAPI.md](https://raw.githubusercontent.com/ORelio/Spotlight-Downloader/master/SpotlightAPI.md)
- **Provenance:** the endpoints were found by analysing Windows network traffic, and the community doc says of the placement ID "Do not change this value". It also says country changes the image set, and that a "No ad available" reply may need a different user agent; I did not hit that with a generic client — [SpotlightAPI.md](https://raw.githubusercontent.com/ORelio/Spotlight-Downloader/master/SpotlightAPI.md)
- **v3:** documented at `arc.msn.com/v3/Delivery/Placement`, `pid=209567`, 1080p maximum — [SpotlightAPI.md](https://raw.githubusercontent.com/ORelio/Spotlight-Downloader/master/SpotlightAPI.md). My one v3 request used the wrong path and returned 500, so v3 is untested, not shown dead.
- **Terms:** no terms specific to this feed exist. The Microsoft Services Agreement bars circumventing "any restrictions on access to, usage, or availability of the Services", citing impermissible scraping as an example (section 3.a.vi) — [Microsoft Services Agreement](https://www.microsoft.com/en-us/servicesagreement). Sampled images are Getty-credited: "leonid_tit / iStock / Getty Images Plus" — Live test

#### Unsplash API
- **Endpoint and auth:** `https://api.unsplash.com`, `Authorization: Client-ID ACCESS_KEY` — [Docs](https://unsplash.com/documentation)
- **Rate limits:** 50 requests/hour in demo mode, 1000/hour after production approval. Requests to `images.unsplash.com` do not count — [Docs](https://unsplash.com/documentation)
- **Search and categories:** `/search/photos`, `/photos/random`, `/topics` and `/topics/:slug/photos`, with `orientation` (`landscape`, `portrait`, `squarish`) and `content_filter` (`low` default, `high`). Random photos cannot combine collections or topics with a query — [Docs](https://unsplash.com/documentation)
- **Image URLs:** `w`, `h`, `crop`, `fm`, `auto=format`, `q`, `fit` and `dpr` are supported, and the `ixid` parameter must be kept on all resized URLs — [Docs](https://unsplash.com/documentation)
- Hotlinking, download tracking, attribution, key handling and the wallpaper ban are in the next section.

#### Pexels API
- **Endpoints:** `GET https://api.pexels.com/v1/search`, `/v1/curated`, `/v1/photos/:id`, `/v1/collections/featured`. Auth is an `Authorization` header — [Docs](https://www.pexels.com/api/documentation/)
- **Rate limits:** 200 requests/hour and 20,000/month by default. `X-Ratelimit-*` headers appear on 2xx responses only. "If you meet our API terms, you can get unlimited requests for free" — [Docs](https://www.pexels.com/api/documentation/)
- **Filters:** `orientation`, `size` (large 24MP, medium 12MP, small 4MP minimum), `color`, `locale`. `src.original` is unmodified. `per_page` maximum is 80 — [Docs](https://www.pexels.com/api/documentation/)
- **Caching:** Pexels' FAQ recommends your own cache and calls 24 hours a reasonable window — [Pexels help, snippet](https://help.pexels.com/hc/de-de/articles/900005880463)
- **Attribution:** "show a prominent link to Pexels" on API use and "Always credit our photographers when possible" — [Docs](https://www.pexels.com/api/documentation/#guidelines)

#### Pixabay API
- **Endpoint and auth:** `GET https://pixabay.com/api/?key=...`. The limit is 100 requests per 60 seconds per key, with HTTP 429 beyond that — [Docs](https://pixabay.com/api/docs/)
- **Resolution:** `largeImageURL` is capped at 1280px. `fullHDURL` (1920px) and `imageURL` (original) are "only available if your account has been approved for full API access" — [Docs](https://pixabay.com/api/docs/)
- **Filters:** `category` (backgrounds, fashion, nature, science, education, feelings, health, people, religion, places, animals, industry, computer, food, sports, transportation, travel, buildings, business, music), `safesearch`, `orientation`, `min_width`, `min_height`, `editors_choice`, `image_type`, `order` — [Docs](https://pixabay.com/api/docs/)
- **Caching and hotlinking:** "requests must be cached for 24 hours" and "permanent hotlinking of images (using Pixabay URLs in your app) is not allowed"; images should be downloaded first. Attribution is required wherever search results are displayed — [Docs](https://pixabay.com/api/docs/)

#### Wallhaven API
- **Endpoint:** `https://wallhaven.cc/api/v1/search`, plus `/w/<ID>`, `/tag/<ID>` and collections. Key is optional, via `?apikey=` or `X-API-Key` — [Docs](https://wallhaven.cc/help/api)
- **Rate limit:** "API calls are currently limited to 45 per minute"; HTTP 429 beyond that — [Docs](https://wallhaven.cc/help/api). Live headers confirmed `X-RateLimit-Limit: 45` — [Live test](https://wallhaven.cc/api/v1/search?q=nature&categories=100&purity=100&atleast=2560x1440&ratios=16x9&sorting=toplist&topRange=1M)
- **Filters:** `categories` (general/anime/people flags), `purity` (sfw/sketchy/nsfw flags), `atleast`, `resolutions`, `ratios`, `colors`, `sorting`, `topRange`, `seed`; 24 per page — [Docs](https://wallhaven.cc/help/api). `ratios=landscape` also worked in a live test, though the docs show only numeric ratios.
- **Rights:** "All images remain property of their original owners." Because content is user-submitted, "some of it will inevitably be the work of others", and rights holders ask for removal by email — [About](https://wallhaven.cc/about). The terms do not address automated access, API use or commercial use — [Terms](https://wallhaven.cc/terms)
- **No licence travels with an image.** One live result's `source` field was a raw Google redirect URL, so it cannot be relied on for attribution — Live test

#### NASA APOD
- **Legacy API is failing now:** `api.nasa.gov/planetary/apod` returned 500 with no date. Five dated requests and `count=2` returned title "NASA Science" with an `hdurl` pointing at a NASA logo PNG, and two earlier dated requests returned today's explanation text under older dates — [Live test](https://api.nasa.gov/planetary/apod?api_key=DEMO_KEY)
- **Retirement:** "The legacy APOD API will be taken offline December 1, 2026. You will need to migrate by then." The catalog also says "concept_tags, hd, and thumbs are now disabled" — [NASA API catalog](https://api.nasa.gov/assets/json/apis.json)
- **Replacement:** `GET https://science.nasa.gov/wp-json/wp/v2/apod-basic` returned 25 items with `date`, `title`, `media_type`, `credit`, `copyright`, `alt`, `hdurl` and `permalink`. It worked with no `api_key`, although the catalog example passes one — [Live test](https://science.nasa.gov/wp-json/wp/v2/apod-basic)
- **Paging:** `?page=2` worked, and `X-WP-Total` was 11457. `?date=` and `?start_date=` were ignored and returned the latest items, so date lookup needs paging — Live test
- **Size:** `hdurl` is a dynamic URL with `w` and `h` set. Today's was a 27 MB PNG at `w=4144&h=2822`, which suits a download budget poorly — Live test
- **Rights:** "Some of the images are copyrighted", and "Neither NASA nor APOD can grant permission to use copyrighted images" — [APOD about page](https://science.nasa.gov/apod/apod-about). Today's image carries a `copyright` value (Artem Mironov), so it is not public domain — Live test
- **Legacy rule of thumb:** "Anything without a copyright returned field is generally NASA and in the public domain" — [nasa/apod-api](https://github.com/nasa/apod-api)

#### NASA Image and Video Library
- **Endpoint:** `https://images-api.nasa.gov/search?q=nebula&media_type=image` returned 200 with no key — [Live test](https://images-api.nasa.gov/search?q=nebula&media_type=image&page_size=2)
- **Assets:** `images-assets.nasa.gov/image/<id>/collection.json` lists `~orig`, `~medium`, `~small` and `~thumb`. The listed URLs are `http://`, and `https://` also worked. One `~orig.jpg` was 2800x1400, so resolution varies — Live test
- **Rights:** NASA content is "generally … not subject to copyright in the United States", and NASA should be acknowledged as the source. NASA logos are not public domain, third-party material may appear, and commercial use must not imply endorsement — [NASA guidelines](https://www.nasa.gov/nasa-brand-center/images-and-media/)

#### Wikimedia Commons
- **Daily feed:** `https://api.wikimedia.org/feed/v1/wikipedia/en/featured/YYYY/MM/DD` returned an `image` object with original URL, size, `artist`, `credit` and `license` — [Live test](https://api.wikimedia.org/feed/v1/wikipedia/en/featured/2026/11/03)
- **Picture of the Day by template:** `action=expandtemplates` on `{{Potd/2026-11-03}}` returned the filename — [Live test](https://commons.wikimedia.org/w/api.php?action=expandtemplates&format=json&prop=wikitext&text=%7B%7BPotd%2F2026-11-03%7D%7D)
- **Metadata:** `prop=imageinfo&iiprop=url|size|extmetadata` returned `LicenseShortName`, `LicenseUrl` and `AttributionRequired` — Live test
- **Thumbnails:** 1920px and 3840px returned 200, but 2560px returned HTTP 400, so only some widths are accepted. `iiurlwidth=3840` returned a working `thumburl` — Live test
- **Rate limits:** anonymous clients get 10/minute when unidentified and 200/minute with a compliant User-Agent. The page says these are new and "subject to experimentation and change" — [Wikimedia rate limits](https://www.mediawiki.org/wiki/Wikimedia_APIs/Rate_limits)
- **User-Agent:** format is `<client name>/<version> (<contact information>)`. Contact can be a website rather than an email. Missing or generic agents can get 403 — [User-Agent policy](https://foundation.wikimedia.org/wiki/Policy:Wikimedia_Foundation_User-Agent_Policy)
- **Attribution:** credit the creator, identify the licence, and link where the licence requires. Hotlinking "is not recommended"; downloading the file is suggested instead — [Commons reuse guide](https://commons.wikimedia.org/wiki/Commons:Reusing_content_outside_Wikimedia). Sampled images were CC BY-SA 4.0 with `AttributionRequired: true` — Live test

#### Reddit
- **Access:** "You must request access and get explicit approval before accessing any Reddit data through our API" — [Responsible Builder Policy, snippet](https://support.reddithelp.com/hc/en-us/articles/42728983564564-Responsible-Builder-Policy). Commercial use also requires permission — same source.
- **Auth and limits:** "100 queries per minute (QPM) per OAuth client id", averaged over 10 minutes. "Traffic not using OAuth or login credentials will be blocked" — [Data API Wiki, snippet](https://support.reddithelp.com/hc/en-us/articles/16160319875092-Reddit-Data-API-Wiki)
- **Pricing:** no official rate card found. Secondary sources report about $12,000/month with 50 million calls and $0.24 per 1,000 beyond that, and the article itself says this is not a published rate card — [Prowlo, a vendor blog](https://prowlo.com/blog/reddit-data-api). A vendor selling an alternative has an interest here.
- **Retention:** the same vendor blog reports a wiki recommendation to delete stored content within 48 hours. A targeted search of Reddit's help site did not confirm it — [Prowlo](https://prowlo.com/blog/reddit-data-api)

#### Flickr
- **Endpoint:** `flickr.photos.search` requires an `api_key`. It offers `tags`, `text`, `license`, `safe_search`, `sort` (including `interestingness-desc`) and `extras` such as `url_o`, `url_l` and `o_dims`. No dimension or orientation filter is documented — [Docs](https://www.flickr.com/services/api/flickr.photos.search.html)
- **Rate limit:** "under 3600 queries per hour across the whole key (which means the aggregate of all the users of your integration)" — [Flickr developer guide](https://www.flickr.com/services/developer/api/)
- **Licence IDs:** 4 CC BY 2.0, 5 CC BY-SA 2.0, 7 no known restrictions, 8 US Government, 9 CC0, 10 Public Domain Mark, 11 CC BY 4.0, 12 CC BY-SA 4.0 — [Docs](https://www.flickr.com/services/api/flickr.photos.licenses.getInfo.html)
- **Terms (updated 9 May 2018):** do not "Cache or store any Flickr user photos other than for reasonable periods", and display "This product uses the Flickr API but is not endorsed or certified by SmugMug, Inc." — [Flickr API terms](https://www.flickr.com/services/api/tos/)

#### Smithsonian Open Access
- **Endpoint:** `https://api.si.edu/openaccess/api/v1.0/search` with an api.data.gov key — [Live test](https://api.si.edu/openaccess/api/v1.0/search?q=landscape&api_key=DEMO_KEY)
- **Limits:** documented as DEMO_KEY 30/hour and 50/day per IP, registered keys 1000/hour — [api.data.gov](https://api.data.gov/docs/developer-manual/). Live `DEMO_KEY` headers showed `X-Ratelimit-Limit: 10`, which conflicts with the manual.
- **Filter:** `q=landscape AND online_media_type:"Images"` gave 11,165 rows. Rights sit per media item at `online_media.media[].usage.access` (`CC0`). At least one returned row had no media, so check client-side — Live test
- **Images:** `https://ids.si.edu/ids/deliveryService?id=<idsId>&max=3840` worked. A 6468x4447 master came back at 4000x2750 with no `max`, and `max=8000` did not exceed that, so delivery looks capped near 4000px — Live test
- **Rights:** CC0 assets may be used commercially "without any attribution, permission, or fee paid to the Smithsonian". Third-party rights and trademarks are outside CC0, and non-CC0 items need written permission — [Smithsonian FAQ, snippet](https://www.si.edu/node/631369); [Terms, snippet](https://www.si.edu/termsofuse)

#### The Met
- **Endpoints:** `/public/collection/v1.1/search` (`offset`, `limit` up to 500, offset plus limit capped at 10,000), `/v1/objects/[id]`, `/v1/departments`. No key: "we do not require API users to register or obtain an API key". "Please limit request rate to 80 requests per second" — [Docs](https://metmuseum.github.io/)
- **v1 search is gone:** the old path returned 410, "retired on 2026-10-01" — [Live test](https://collectionapi.metmuseum.org/public/collection/v1/search?q=landscape&hasImages=true)
- **`isPublicDomain` does not filter search:** `q=landscape&hasImages=true` gave 34,669 with and without `isPublicDomain=true`. The docs do not list that parameter, so check it per object — Live test
- **Images:** `primaryImage` is under `/original/` and was 4000x3184 for the sample. Objects carry `tags`, `classification` and `department`, and no pixel dimensions — [Live test](https://collectionapi.metmuseum.org/public/collection/v1/objects/436535)
- **Rights:** public-domain images are offered under CC0, with more than 406,000 available — [Met image resources, snippet](https://www.metmuseum.org/en/policies/image-resources). The API dataset is CC0 and includes identifying data for works still under copyright — [Docs](https://metmuseum.github.io/)

#### Art Institute of Chicago
- **Endpoint:** `https://api.artic.edu/api/v1/artworks/search`, no key. Live `x-ratelimit-limit: 60` — Live test. Third-party summaries say that is per minute per IP — [api-evangelist file](https://raw.githubusercontent.com/api-evangelist/art-institute-of-chicago/refs/heads/main/rate-limits/art-institute-of-chicago-rate-limits.yml). I could not read the official docs to confirm the window.
- **Images need the courtesy header:** from a script client, IIIF image requests returned 200 with an `AIC-User-Agent` header and 403 without it, alternating across four tries — Live test against `https://www.artic.edu/iiif/2/{image_id}/full/843,/0/default.jpg`
- **Image size:** 843, 1686 and 3000 wide all worked from a 5898x4176 master. Pixel size is in `thumbnail.width` and `thumbnail.height` — Live test
- **Search behaviour:** `q` with a public-domain filter ranks by relevance but does not narrow `pagination.total` (59,060 for every term), so the total cannot be used as a count. `subject_titles` values such as `landscapes`, `cityscapes`, `abstract` and `animals` appear on records — Live test
- **Rights:** the API states that `description` is CC-BY and all other response data is CC0 — Live test (`info.license_text`). Public-domain images are offered under CC0 — [AIC open access, snippet](https://www.artic.edu/open-access/open-access-images). Data dumps exist — [api-data repo](https://raw.githubusercontent.com/art-institute-of-chicago/api-data/master/README.md)

#### Rijksmuseum
- **Old API is gone:** `rijksmuseum.nl/api/en/collection` returned 410 — Live test
- **New API:** `https://data.rijksmuseum.nl/search/collection?type=painting&imageAvailable=true` returned 200 with no key, 4,924 items, up to 100 per page, and token paging through `next.id`. Parameters: `title`, `creator`, `type`, `material`, `technique`, `imageAvailable`, `description`, `aboutActor` — [Live test](https://data.rijksmuseum.nl/search/collection?type=painting&imageAvailable=true); [Docs](https://data.rijksmuseum.nl/docs/search)
- **Getting images:** search returns only IDs. Resolving `https://id.rijksmuseum.nl/<id>` with `Accept: application/ld+json` returned a `https://iiif.micr.io/<code>/full/max/0/default.jpg` URL and a rights URI. One image was 5862x5197 — Live test; [IIIF docs](https://data.rijksmuseum.nl/docs/iiif/image)
- **Rate limits:** not stated on the search page — [Docs](https://data.rijksmuseum.nl/docs/search)
- **Rights:** out-of-copyright material carries "a Public Domain Mark (PDM) and/or the Creative Commons Zero 1.0 (CC0 1.0)". Where the museum exercises copyright it uses CC BY 4.0. "All users, including commercial parties, have access to the same Information and Data under the same conditions." Attribution is called "good practice" regardless of licence — [Rijksmuseum data policy](https://data.rijksmuseum.nl/policy/information-and-data-policy)

#### National Gallery of Art
- **No search API found.** Data is CSV, CC0, "usually once a day" — [NGA opendata](https://github.com/NationalGalleryOfArt/opendata)
- **`published_images.csv`** is 92 MB with 129,660 rows, including `iiifurl`, `width`, `height`, `openaccess`, `viewtype`, `depictstmsobjectid` and `assistivetext`. 63,954 rows are open-access primary images, and 21,105 of those are landscape format at 3840px or wider — [Live test](https://raw.githubusercontent.com/NationalGalleryOfArt/opendata/main/data/published_images.csv)
- **IIIF:** `https://api.nga.gov/iiif/<uuid>/full/3840,/0/default.jpg` returned 3840x3011. `full` and `max` returned 4096x3212 from a 4490x3521 master, so delivery looks capped at 4096 — Live test
- **Rights:** open-access images are "free of charge for any use, commercial or non-commercial" under CC0. Users may not imply endorsement, and a "Courtesy National Gallery of Art" credit is requested — [NGA open access, snippet](https://www.nga.gov/notices/open-access-policy.html)

#### National Geographic Photo of the Day
- **Status conflict:** KDE removed its provider in August 2024, saying the feed had not updated since 2022 and the service appeared to be discontinued — [KDE bug 491686](https://bugs.kde.org/show_bug.cgi?id=491686). Yet the web page is live, carries dates up to 3 November 2026, and served a 3072x2049 image modified that day — [Live test](https://www.nationalgeographic.com/photo-of-the-day)
- **Reconciliation:** the old feed appears dead while a relaunched editorial page runs. I found no API.
- **Terms:** the National Geographic Society's terms (updated 19 November 2025) have a "Prohibition on Data Scraping" section — [NatGeo Society terms, snippet](https://nationalgeographic.org/terms-of-service). That is the .org site, and I could not read the .com terms.

#### Google Earth View
- **Site:** `earthview.withgoogle.com` now redirects to a Chrome Web Store extension last updated 3 July 2024 — [Live test](https://earthview.withgoogle.com/)
- **Assets:** `gstatic.com/prettyearth/assets/data/v3/<id>.json` and `/full/<id>.jpg` still work, at 1800x1200. IDs are sparse: 1003, 1004 and 2000 existed, 1200 and 5000 did not — [Live test](https://www.gstatic.com/prettyearth/assets/full/1003.jpg). A community list has 1,520 entries — [limhenry/earthview](https://github.com/limhenry/earthview)
- **Terms:** "Google Earth content may not be used for any commercial or promotional purposes" and "Google Earth may not be embedded online or in apps". Attribution to Google and data providers must stay with the content. Earth View is not named — [Google geo guidelines](https://about.google/brand-resource-center/products-and-services/geo-guidelines/)

#### Apple aerials
- The catalogue at `sylvan.apple.com` holds video, used by community screensaver ports — [Aerial projects](https://github.com/OrangeJedi/Aerial). The TLS certificate chains to "Apple Server Authentication CA", which failed default validation here, so I did not read the file. I found no licence for third-party use.

#### Others worth knowing
- `source.unsplash.com` returned 503 — [Live test](https://source.unsplash.com/1600x900/?nature). Old guides relying on it are out of date.
- Lorem Picsum works keylessly, with Unsplash-derived images up to 5000x3333 — [Live test](https://picsum.photos/v2/list?limit=2). I did not review its terms.

### Inferences
- Bing and Spotlight share one legal shape: unofficial access, Getty-owned content, and a Microsoft clause limiting use to noncommercial personal use. A free app that fetches on the user's device sits closer to that clause than a paid one, but that is my reading and needs a lawyer.
- The Earth View assets are a legacy leftover. The low resolution and Google's terms both argue against it, regardless of whether the URLs keep working.
- The APOD case shows why endpoints should be changeable without an app update: a date-specific request now silently returns a NASA logo with HTTP 200. Validating that an image is a plausible wallpaper matters more than checking the status code.

### Gaps
- Official docs or policy pages I could not read, despite trying: AIC docs and terms, Smithsonian FAQ and API docs, Met image policy (429), NGA policy, Pexels terms and help, and Reddit's Data API Terms. Those findings rest on snippets or secondary sources. A person with a browser should confirm them.
- Reddit was unreachable from this environment, so nothing about it is live-tested. Reddit's pricing is unverified.
- The NASA Image Library's rate limit was not found. Rijksmuseum and NGA publish none on the pages I read.
- AIC's IIIF size guidance is unverified, as is whether the 403 depends on Cloudflare rules that change over time.
- Live results are a single day's snapshot. Bing and Spotlight can change without notice.

## Which sources forbid or restrict automated desktop-wallpaper use, require per-user keys, or forbid shipping an embedded key?

### Takeaway
Unsplash and Pexels both name wallpaper apps as prohibited, and Pixabay's licence restricts distributing images as wallpaper, so none of the three is a safe foundation for backreel. Unsplash, api.data.gov keys and probably any shared key cannot be embedded in a distributed client.

### Cited Findings

#### Unsplash, checked in detail
- **Wallpaper ban:** Usage Guideline 3 says you may not reproduce Unsplash's core user experience, listing "unofficial clients, wallpaper applications, etc." — [API Guidelines](https://help.unsplash.com/en/articles/2511245-unsplash-api-guidelines)
- **Hotlinking:** "All API uses must use the hotlinked image URLs returned by the API under the `photo.urls`" — [API Guidelines](https://help.unsplash.com/en/articles/2511245-unsplash-api-guidelines). The API terms say "you must directly use or embed the related image URLs returned by the API" (section 6) — [API Terms](https://unsplash.com/api-terms)
- **Download tracking:** when an app does something like a download, it must call the endpoint at `photo.links.download_location` — [API Guidelines](https://help.unsplash.com/en/articles/2511245-unsplash-api-guidelines). The terms name the wallpaper case: "each time there is a download of an Image or other comparable event (e.g., setting an Image as background wallpaper)" (section 6) — [API Terms](https://unsplash.com/api-terms). The endpoint is "for tracking purposes only" and not for embedding — [Docs](https://unsplash.com/documentation)
- **Attribution:** the app "must attribute Unsplash, the Unsplash photographer, and contain a link back to their Unsplash profile", with `utm_source=<app>&utm_medium=referral` on links — [API Guidelines](https://help.unsplash.com/en/articles/2511245-unsplash-api-guidelines)
- **Keys:** Access Key and Secret Key must stay private, and client-side apps may need a proxy — [API Guidelines](https://help.unsplash.com/en/articles/2511245-unsplash-api-guidelines). "You may not share your Credentials with any third party" (section 2) — [API Terms](https://unsplash.com/api-terms). The docs add that for decentralised apps, "a single API key can't be shared between all installations" — [Docs](https://unsplash.com/documentation)
- **Production approval:** submissions must clearly show every guideline is met, with examples, or the limit increase will likely be rejected — [API Guidelines](https://help.unsplash.com/en/articles/2511245-unsplash-api-guidelines)
- **Licence:** "This license does not include the right to compile images from Unsplash to replicate a similar or competing service" — [Unsplash License](https://unsplash.com/license)

#### Other restrictions

| Source | Restriction | Citation |
|---|---|---|
| Pexels | "You may not copy or replicate core functionality of Pexels (including making Pexels content available as a wallpaper app)." | [Docs](https://www.pexels.com/api/documentation/#guidelines) |
| Pexels | Wallpaper apps barred when they distribute content Standalone or compile it to replicate a competing service | [Help, snippet](https://help.pexels.com/hc/de-de/articles/900005880463) |
| Pixabay | "You cannot sell or distribute the Content (either in digital or physical form) on a Standalone basis"; original form, or only a filter, resize or crop, counts as Standalone | [Terms](https://pixabay.com/service/terms/) |
| Pixabay | Bars selling or distributing content "as a print, wallpaper, poster or on merchandise or on other physical products"; bulk or systematic copying needs permission | [Terms](https://pixabay.com/service/terms/) |
| Flickr | No app that "replicates or attempts to replace the essential user experience of Flickr.com"; commercial use "only with prior permission" | [API terms](https://www.flickr.com/services/api/tos/); [API keys](https://www.flickr.com/services/api/misc.api_keys.html) |
| api.data.gov (Smithsonian, NASA) | The key "Should be kept private and should not be shared" | [Manual](https://api.data.gov/docs/developer-manual/) |
| Bing, Spotlight | Noncommercial personal use only; no circumventing access restrictions | [Microsoft Services Agreement](https://www.microsoft.com/en-us/servicesagreement) |
| Google Earth | No commercial use; not to be embedded in apps | [Geo guidelines](https://about.google/brand-resource-center/products-and-services/geo-guidelines/) |
| Reddit | Explicit approval first; permission for commercial use | [Policy, snippet](https://support.reddithelp.com/hc/en-us/articles/42728983564564-Responsible-Builder-Policy) |

### Inferences
- There is a tension in Unsplash's documents: the guidelines name wallpaper applications as barred, while the terms treat setting a wallpaper as a tracked download event. The likeliest reading is that an app may let users set an occasional Unsplash image as wallpaper, while an app whose main job is delivering wallpapers is what's barred. backreel's core function falls on the barred side.
- Windows needs a local file to set a wallpaper, so the hotlink rule cannot be followed literally on the desktop. That is one more mismatch with Unsplash, on top of the explicit ban.
- Bring-your-own-key moves the credential but does not remove the product restriction. The rule targets what the app is, so a user-supplied key does not obviously cure it. I would not build on that argument without Unsplash or Pexels agreeing in writing.
- The Pixabay "wallpaper" clause lists physical goods alongside wallpaper, so it could be read narrowly. The Standalone clause covers digital distribution anyway. An app that delivers unmodified images one at a time from Pixabay's API to a single user is an unclear case rather than a clear ban.
- Asking Pexels directly could be worthwhile, since it offers unlimited requests to compliant apps, but the wallpaper line is explicit.

### Gaps
- No provider's wording addresses a client-only app that downloads straight from the provider to one user's device. This is the central unknown for Pixabay.
- I found no statement from Pexels, Pixabay, Flickr or Wallhaven on embedding keys in client apps.
- Flickr key eligibility is disputed: one 2026 blog says new keys need Pro — [AWP Life](https://awplife.com/how-to-get-flickr-api-key/) — while Flickr's own pages I read do not mention that.

## How should API keys be handled in a desktop app with no backend, and what does each option cost?

### Takeaway
Ship keyless first. Add a small config manifest (static or Worker-served) so endpoints can change without an app update, and offer bring-your-own-key only for optional sources whose terms allow it. A thin proxy costs nothing to about $5 a month in hosting, but it does not fix the wallpaper-app bans.

### Cited Findings
- Cloudflare Workers Free includes 100,000 requests/day; over the limit, requests fail. Paid is a $5/month minimum with 10 million requests, then $0.30 per million, with no egress charge — [Workers pricing](https://developers.cloudflare.com/workers/platform/pricing/)
- KV on Free allows 100,000 reads and 1,000 writes per day and 1 GB of storage. Paid includes 10 million reads a month — [Workers pricing](https://developers.cloudflare.com/workers/platform/pricing/)
- Unsplash says client-side apps may need a proxy to keep keys hidden — [API Guidelines](https://help.unsplash.com/en/articles/2511245-unsplash-api-guidelines)
- Image file requests to `images.unsplash.com` do not count against API limits — [Docs](https://unsplash.com/documentation)
- A shared key's limit covers every install: Flickr's 3600/hour is "the aggregate of all the users of your integration" — [Flickr](https://www.flickr.com/services/developer/api/). Pexels' default is 20,000/month — [Docs](https://www.pexels.com/api/documentation/)

### Inferences

| Option | Money | Other cost | Fits |
|---|---|---|---|
| Keyless only | $0 | Fewer sources; no "minimal" coverage without Wallhaven | Default install |
| Bring your own key | $0 to you | Sign-up friction, so few users will do it; each user holds their own quota; product restrictions still apply | Optional power-user sources such as Wallhaven accounts, Smithsonian, Flickr |
| Thin proxy | $0 up to 100,000 requests/day; then about $5/month | You run a service, hold keys, need abuse controls, and see user IPs, which needs a privacy note | Keys that must stay secret, shared caching |
| Static manifest | About $0 on any static host | No key protection, but gives remote endpoint fixes and pre-built indexes | Endpoint changes; NGA index |

- Sizing example, my assumption: 3 metadata requests per install per day means the free Worker tier covers about 33,000 daily active installs, and the $5 tier's 10 million monthly requests covers about 110,000. Image bytes should go straight from provider CDNs, not through the proxy.
- With edge caching, upstream calls stay low. Refreshing 8 categories hourly is 5,760 calls a month, within Pexels' default, if its terms allowed use at all.
- A proxy that caches and serves images would make you a redistributor, which is a worse position than one fetching metadata only. Keep it metadata-only.
- A static manifest would have made the APOD migration a config change rather than an app release. It is the best value item here even if no proxy is ever built.
- The Smithsonian adds little that the Met, AIC, Rijksmuseum and NGA do not, so it does not justify a proxy on its own.

### Gaps
- I did not check static hosting limits or terms (GitHub Pages, Cloudflare Pages or R2).
- Install-count and request assumptions are illustrative, not measured.

## Which sources need no key at all, so that the default install works with zero configuration?

### Takeaway
Ten sources returned usable data with no registered key today. Seven of those have clear reuse terms: Wikimedia, NASA Image Library, APOD (non-copyright items only), the Met, AIC, Rijksmuseum, and NGA through its CSV data.

### Cited Findings

| Source | Confirmed keyless today | Licence clarity | Note |
|---|---|---|---|
| Wikimedia feed and Commons | [Yes](https://api.wikimedia.org/feed/v1/wikipedia/en/featured/2026/11/03) | Clear, per file; attribution needed | Needs a compliant User-Agent |
| NASA Image Library | [Yes](https://images-api.nasa.gov/search?q=nebula&media_type=image&page_size=2) | Clear, some exceptions | |
| NASA APOD new endpoint | [Yes](https://science.nasa.gov/wp-json/wp/v2/apod-basic) | Per item, check `copyright` | Docs show an `api_key` parameter |
| The Met | [Yes](https://collectionapi.metmuseum.org/public/collection/v1/objects/436535) | Clear per object | Check `isPublicDomain` per object |
| Art Institute of Chicago | [Yes](https://api.artic.edu/api/v1/artworks/search?q=landscape&limit=1) | Clear for public-domain works | `AIC-User-Agent` needed for images |
| Rijksmuseum | [Yes](https://data.rijksmuseum.nl/search/collection?type=painting&imageAvailable=true) | Clear per object | Extra hop per object |
| National Gallery of Art | [Yes](https://raw.githubusercontent.com/NationalGalleryOfArt/opendata/main/data/published_images.csv) | Clear | 92 MB CSV, needs an index |
| Bing | [Yes](https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=1&mkt=en-US) | Restricted | Unofficial |
| Windows Spotlight | [Yes](https://fd.api.iris.microsoft.com/v4/api/selection?placement=88000820&bcnt=4&country=US&locale=en-US&fmt=json) | Restricted | Unofficial |
| Wallhaven (SFW) | [Yes](https://wallhaven.cc/api/v1/search?q=nature&categories=100&purity=100) | Unclear per image | User-uploaded |

- `DEMO_KEY` is a poor substitute: it was limited to 10 in live headers and hit 429 during testing, and limits apply per IP — Live test; [api.data.gov](https://api.data.gov/docs/developer-manual/)

### Inferences
- For a production Wikimedia User-Agent, use the project URL as the contact, such as `backreel/1.0 (https://<project-site>)`. That meets the policy without any personal email.
- A generic `AIC-User-Agent` value was enough in testing. A third-party summary describes the header as an app name plus contact, so a project URL fits there too.

### Gaps
- Whether these keyless paths stay open at scale is unknown. A popular app could trigger bot defences, as AIC's 403 behaviour suggests.

## How can categories be unified across sources that do not share a taxonomy?

### Takeaway
Use a fixed list of backreel categories, and give each source an adapter that translates the category into its own query. Treat daily feeds as a separate "daily" mode with a local keyword classifier, because Bing, Spotlight and APOD have no categories. Keep subject, style and medium as separate internal facets so "art" and "minimal" do not collide with "nature".

### Cited Findings

Counts below are live results on 2026-11-04, and show which queries return enough images. Wallhaven counts used `categories=100&purity=100&atleast=2560x1440&ratios=landscape`. Pixabay, Unsplash and Pexels rows are not live-tested.

| Category | Wikimedia Commons (`incategory:`) | NASA Images (`q=`) | Met (`tags=true&hasImages=true`) | AIC (`subject_titles`, ranked by `q`) | Rijksmuseum | Smithsonian (`topic:`) | Wallhaven (`q=`) | Pixabay (docs only) |
|---|---|---|---|---|---|---|---|---|
| Nature | `Featured pictures of landscapes` (492), `...of plants` (53) | `earth from space` (19,471) | `q=landscape` (5,691), `q=flowers` (16,568) | `landscapes` | `type=painting&description=landscape` (115) | `Landscapes` (6,871) | `nature` (25,088), `landscape` (17,417) | `category=nature` |
| Space | `Featured pictures of astronomy` (102), `...of space exploration` (10) | `nebula` (316), `galaxy` (1,747) | None | None | None | `Astronomy` (228) | `space` (4,812) | `q=space&category=science` |
| Architecture | `Featured pictures of architecture` (49) | None | `q=Architecture` (4,504) | `architecture` | Sparse | `Architecture` (11,381) | `architecture` (2,781) | `category=buildings` |
| Art | `Featured pictures of paintings` (3, needs subcategories) | None | `departmentId=11` (European Paintings) | Any public-domain work | `type=painting` (4,924) | Not mapped | `painting` (3,221), `digital art` (32,353) | `image_type=illustration` |
| Animals | `Featured pictures of animals` (4 direct; subcategories hold the rest) | None | `q=Animals` (4,537) | `animals` | `type=print&description=animal` (9) | `Animals` (1,130,168, likely specimens) | `animals` (5,833) | `category=animals` |
| Abstract | None | None | `q=Abstraction` (1,653) | `abstract` | None | `Abstract` returned 0: do not use | `abstract` (5,801) | `q=abstract&category=backgrounds` |
| City | `Featured pictures of cityscapes` (520) | None | `Cityscapes` returned 0: use `q=city` untagged, untested | `cityscapes` | Sparse | Not mapped | `city` (8,415) | `category=places&q=city` |
| Minimal | None | None | None | None | None | None | `minimalism` (6,929) | `q=minimal&category=backgrounds` |

- Commons counts are direct members only. `Category:Featured pictures of cities` does not exist, while `...of cityscapes` does. The parent `Category:Featured pictures on Wikimedia Commons` has 22,703 files — [Live test](https://commons.wikimedia.org/w/api.php?action=query&format=json&prop=categoryinfo&titles=Category:Featured_pictures_of_landscapes)
- Commons search supports dimension filtering: `incategory:"Featured pictures of landscapes" filew:>3839 filemime:image/jpeg` returned 458 hits — Live test
- Pixabay's categories are fixed — [Docs](https://pixabay.com/api/docs/). Wallhaven tags are free-form with operators — [Docs](https://wallhaven.cc/help/api). Met tags link to Getty AAT and Wikidata IDs, for example "Landscapes" to `aat/300132294` — Live test
- Only one Unsplash topic slug was verified (`textures-patterns`). The site menu shows Wallpapers, Nature, Textures, Architecture and others, but the slugs are unconfirmed — [Unsplash topic page](https://unsplash.com/t/textures-patterns). `GET /topics` would list them with a key — [Docs](https://unsplash.com/documentation)
- Bing's response has no category field. Its `title` and `copyright` text are the only hints, as with "Lake Braies, Fanes-Sennes-Braies Nature Park, Dolomites, Italy" — Live test. Spotlight likewise offers `title` and `description` — Live test

### Inferences

**Recommended model:**

1. **Fixed user-facing list:** Nature, Space, Architecture, Art, Animals, Abstract, City, Minimal, plus a "Daily picks" toggle.
2. **Three internal facets:**
   - `subject`: nature, space, architecture, animals, city
   - `style`: abstract, minimal
   - `medium`: photo, painting, print, illustration
   
   A Monet landscape is `subject=nature, medium=painting`. It can then match both Art and Nature, and users who want photographs can exclude paintings.
3. **Adapter per source:** `SourceAdapter.Fetch(category, constraints)` returns candidates. The mapping table lives in the remote manifest, not in code, so a query that returns nothing can be fixed without a release.
4. **Normalised record:** `source, sourceId, imageUrl, width, height, licence, creator, creditText, sourceUrl, subject[], style[], medium, confidence, fetchedAt`.
5. **Confidence:** curated mappings (a Commons category, a Met tag) score high. Keyword hits score low. Feeds classified by the keyword matcher score lowest, and match a category only if the user has also enabled "Daily picks".
6. **Coverage rule:** a category needs at least two sources or a large pool before it is offered in a default install.

**Coverage consequences:**

- Nature, Architecture, Art, Animals and City are coverable with keyless open sources.
- Space is well served by NASA alone.
- Abstract relies on museums, so it means abstract art rather than abstract wallpaper patterns.
- Minimal has no open-licence source. Either make it available only when Wallhaven is enabled, or drop it from v1.
- Smithsonian's huge `Animals` count is likely dominated by specimen records, so raw counts should not be taken as wallpaper-quality supply.
- Museum search recall varies a lot. Rijksmuseum's `description` search is thin, so build a small curated ID list for it instead of searching live.

### Gaps
- Nothing here measures how many results actually look good as wallpapers. A manual review of 50 images per category and source would answer that.
- Commons subcategory recursion (`deepcat:`) was not tested, and the Animals category needs it.
- The Met `q=city` fallback, and the AIC per-subject pool sizes, are unmeasured because AIC's totals do not narrow.
- The Unsplash and Pexels rows are left out on purpose, since those sources are not recommended.

## What content-safety and quality filters are available?

### Takeaway
Only commercial stock and wallpaper sources offer real safety flags. Museums and Wikimedia have none, so backreel needs its own deny-list and a local size and aspect check on every image, whichever source it came from.

### Cited Findings

| Source | Safety control | Resolution filter | Orientation or aspect | Pixel size known before download |
|---|---|---|---|---|
| Wallhaven | `purity=100`; guests got SFW only even when asking for `111`, and `purity=001` returned nothing ([Live test](https://wallhaven.cc/api/v1/search?purity=111)) | `atleast`, `resolutions` | `ratios` | Yes |
| Unsplash | `content_filter=high`, not guaranteed ([Docs](https://unsplash.com/documentation)) | None found | `orientation` | Yes in photo objects (not tested) |
| Pexels | None documented ([Docs](https://www.pexels.com/api/documentation/)) | `size` | `orientation` | Not checked |
| Pixabay | `safesearch=true` ([Docs](https://pixabay.com/api/docs/)) | `min_width`, `min_height` | `orientation` | Yes (`imageWidth`, `imageHeight`) |
| Flickr | `safe_search`; unauthenticated calls see safe content only ([Docs](https://www.flickr.com/services/api/flickr.photos.search.html)) | None documented | None documented | Via `o_dims` |
| Wikimedia | None found | `filew:`, `fileh:` in search (Live test) | Via `filew`/`fileh` | Yes |
| NASA Images | None found | None | None | No in search results |
| The Met | None found | None | None | No |
| AIC | None found; `subject_titles` usable for deny terms | None | None | Yes (`thumbnail`) |
| Rijksmuseum | None found | None | None | Via `info.json` |
| NGA | None found; `assistivetext` usable for deny terms | CSV `width`, `height` | CSV | Yes |
| Smithsonian | Culturally sensitive items may be withheld ([FAQ, snippet](https://www.si.edu/node/631369)) | None found | None | Yes (`resources[].dimensions`) when present |
| Bing, Spotlight | Editorial selection only | Fixed sizes | 16:9 and 9:16 versions | Yes |

- The docs say accessing a NSFW wallpaper without a valid key returns 401 — [Wallhaven docs](https://wallhaven.cc/help/api). My guest search got 200 with those results filtered out, so check for empty lists as well as errors — Live test
- Museum collections are mixed. An unfiltered Smithsonian search for `landscape` led with a book record — Live test
- Many museum works are portrait format: one Smithsonian result was 1922x3200, and NGA has 30,134 open-access primary images at 3840px or wider, of which 21,105 are landscape — Live test

### Inferences

**Baseline filter for every source (my recommended starting values, to tune by testing):**

1. **Format:** still images only. Reject video and `iframe` items, which APOD can return.
2. **Sanity check:** decode the file and check it is plausibly a photo or artwork. Reject tiny files and PNG logos, which would have caught the APOD failure.
3. **Size:** width at least the monitor's width × 0.75, with a hard floor of 1920px.
4. **Aspect:** prefer images within about 25% of the monitor's ratio. For paintings, offer a fit mode with a colour-matched border, since cropping a painting can ruin it.
5. **Download budget:** skip or request a resized version if over about 15 MB. APOD's PNG was 27 MB.
6. **Safety:** deny-list terms against title, tags, subjects and alt text, such as `nude`, `nudity`, `erotic`, `corpse`, `execution`, `crucifixion`, `war`, `weapon`. Add a user switch to tighten it, and a one-click "never show again" that records the source and ID locally.
7. **Licence gate:** require a recognised open licence for default-on sources. Items with an unknown licence never appear by default.

**Other points:**

- Classical art includes nudity and violence, and a daily Commons picture can be anything. Neither is covered by any API flag, so the deny-list is the only defence. Since a wallpaper may be visible at work, a "work-safe" mode should default to on.
- A deny-list will miss items with sparse metadata. Consider showing the next image from daily feeds as a preview that the user can skip before it is applied.
- An on-device image classifier would be stronger than word lists, but it is extra weight for a minimal app, so I would leave it for later.

### Gaps
- I found no published safety guarantees for museum or Wikimedia content, and did not read Commons' own content policy.
- Reddit's `over_18` flag is commonly used, but I could not verify current API behaviour.
- The suggested thresholds are design judgement, not measured values.

## Which sources should ship first?

### Takeaway
Ship v1 with Wikimedia Commons, NASA (Image Library plus new APOD), the Met and AIC on by default, then add Rijksmuseum and NGA once a curated index exists. Offer Bing as an opt-in only after a legal check. Do not ship Unsplash, Pexels, Reddit, Earth View, NatGeo or Apple.

### Cited Findings
All facts behind this ranking are cited in the sections above. The main ones are: wallpaper bans at [Unsplash](https://help.unsplash.com/en/articles/2511245-unsplash-api-guidelines) and [Pexels](https://www.pexels.com/api/documentation/#guidelines), the [Microsoft personal-use clause](https://www.microsoft.com/en-us/servicesagreement), the [APOD retirement notice](https://api.nasa.gov/assets/json/apis.json), and the open terms at [Rijksmuseum](https://data.rijksmuseum.nl/policy/information-and-data-policy) and the [Met](https://metmuseum.github.io/).

### Inferences

| Phase | Source | Default | Why |
|---|---|---|---|
| v1 | Wikimedia Commons | On | Daily picture, photos across most categories, licence data per file |
| v1 | NASA Image Library | On | Keyless, simple, strong for Space |
| v1 | NASA APOD (`apod-basic`) | On, no-`copyright` items only | Recognisable daily feature; use only the new endpoint |
| v1 | The Met | On | Keyless, 4000px class images, good tags |
| v1 | Art Institute of Chicago | On | Good subject data, confirmed pixel sizes; send the header |
| v1 | Bing | Off, user opt-in, after legal review | Best daily photo quality, but unofficial and personal-use only. Don't use in a paid product without permission |
| v1.1 | Rijksmuseum | On | Excellent resolution; needs curated IDs |
| v1.1 | National Gallery of Art | On | 21,105 large landscape-format images; needs a pre-built index |
| v1.1 | Wallhaven | Off, user opt-in with a rights notice | Only route to Minimal; SFW forced; image rights unclear |
| Later | Smithsonian | Off, own key | Adds little over the other museums |
| Later | Flickr | Off, own key, CC0/PD licences only | Possible for a keen minority |
| Later or never | Windows Spotlight | Off | Same legal shape as Bing, with an ad-delivery endpoint that seems less stable |
| Do not ship | Unsplash, Pexels | n/a | Wallpaper apps named as barred |
| Do not ship | Pixabay | n/a unless clarified | Licence risk, and 1280px by default is too small |
| Do not ship | Reddit | n/a | Approval needed, user content with unknown rights |
| Do not ship | Earth View, NatGeo, Apple | n/a | Terms or format rule them out |

**Build requirements that follow:**

- Remote manifest for endpoints and category mappings, from day one.
- Per-source health checks, with silent fallback to another source when one fails or returns junk.
- Local image cache, with attribution stored alongside each file, and a "who made this?" panel. This satisfies Wikimedia's requirements and the museums' credit requests in one feature.
- A compliant `User-Agent` with a project URL, plus `AIC-User-Agent`.

**The main product risk:** without Bing, the default daily image leans on Wikimedia and museums. That gives strong art and decent photography, but it is a different feel from Bing-style glossy landscapes. Check whether that is acceptable before committing. It also leaves Minimal unavailable in a default install.

### Gaps
- The Bing recommendation depends on a legal view I cannot provide. If the app will always be free, ask a lawyer whether the personal-use clause covers it. If it may be paid, ask Microsoft.
- The ranking does not reflect measured image appeal; see the manual review noted in the categories section.
