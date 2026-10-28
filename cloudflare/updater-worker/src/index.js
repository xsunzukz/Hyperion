const MANIFEST_URL =
  'https://github.com/xsunzukz/Hyperion/releases/latest/download/latest.json';
const CACHE_SECONDS = 300;

const json = (body, status = 200, maxAge = 0) =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      'content-type': 'application/json; charset=utf-8',
      'cache-control': maxAge > 0 ? `public, max-age=${maxAge}` : 'no-store',
      'access-control-allow-origin': '*',
    },
  });

export default {
  async fetch(request) {
    const url = new URL(request.url);

    if (url.pathname === '/health') {
      return json({
        ok: true,
        service: 'hyperion-updater',
        manifest: MANIFEST_URL,
      });
    }

    if (url.pathname !== '/' && url.pathname !== '/latest.json') {
      return json({ error: 'Not found' }, 404);
    }

    let upstream;
    try {
      upstream = await fetch(MANIFEST_URL, {
        headers: {
          accept: 'application/json',
          'user-agent': 'Hyperion-Updater/1.0',
        },
        cf: {
          cacheTtl: CACHE_SECONDS,
          cacheEverything: true,
        },
      });
    } catch (error) {
      return json({ error: 'Update manifest could not be reached', detail: String(error) }, 502);
    }

    if (!upstream.ok) {
      return json(
        {
          error: 'No update manifest is published yet',
          upstreamStatus: upstream.status,
        },
        503
      );
    }

    const body = await upstream.text();
    try {
      const parsed = JSON.parse(body);
      if (!parsed.version || !parsed.platforms) {
        throw new Error('missing version or platforms');
      }
    } catch (error) {
      return json({ error: 'Update manifest is invalid', detail: String(error) }, 502);
    }

    return new Response(body, {
      status: 200,
      headers: {
        'content-type': 'application/json; charset=utf-8',
        'cache-control': `public, max-age=${CACHE_SECONDS}`,
        'access-control-allow-origin': '*',
        'x-updater-source': 'github-releases',
      },
    });
  },
};
