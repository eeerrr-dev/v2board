const SCROLLING_EFFECT_CLASS = 'ant-scrolling-effect';

function lockLegacyBodyScrollingEffect() {
  const body = document.body;
  const prevClassName = body.className;
  const prevOverflow = body.style.overflow;
  const prevOverflowX = body.style.overflowX;
  const prevOverflowY = body.style.overflowY;
  const prevPosition = body.style.position;
  const prevWidth = body.style.width;

  const needsScrollbarEffect =
    body.scrollHeight > (window.innerHeight || document.documentElement.clientHeight) &&
    window.innerWidth > body.offsetWidth;
  const scrollbarWidth = needsScrollbarEffect
    ? window.innerWidth - document.documentElement.clientWidth
    : 0;

  if (scrollbarWidth > 0) {
    body.style.position = 'relative';
    body.style.width = `calc(100% - ${scrollbarWidth}px)`;
    if (!body.classList.contains(SCROLLING_EFFECT_CLASS)) {
      body.className = `${body.className} ${SCROLLING_EFFECT_CLASS}`.trim();
    }
  }

  body.style.overflow = 'hidden';
  body.style.overflowX = 'hidden';
  body.style.overflowY = 'hidden';

  return () => {
    body.className = prevClassName;
    body.style.overflow = prevOverflow;
    body.style.overflowX = prevOverflowX;
    body.style.overflowY = prevOverflowY;
    body.style.position = prevPosition;
    body.style.width = prevWidth;
  };
}

export function lockLegacyModalBodyScroll() {
  return lockLegacyBodyScrollingEffect();
}

export function lockLegacyDrawerBodyScroll() {
  const body = document.body;
  const prevTouchAction = body.style.touchAction;
  const unlockScrollingEffect = lockLegacyBodyScrollingEffect();
  body.style.touchAction = 'none';

  return () => {
    unlockScrollingEffect();
    body.style.touchAction = prevTouchAction;
  };
}
