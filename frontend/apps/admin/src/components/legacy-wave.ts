export function triggerLegacyWave(node: HTMLElement) {
  const style = getComputedStyle(node);
  const waveColor =
    style.getPropertyValue('border-top-color') ||
    style.getPropertyValue('border-color') ||
    style.getPropertyValue('background-color');
  const rgb = waveColor.match(/rgba?\((\d+),\s*(\d+),\s*(\d+)/);
  const meaningful =
    !!waveColor &&
    waveColor !== 'transparent' &&
    waveColor !== 'rgb(255, 255, 255)' &&
    !/rgba\((?:\d+,\s*){3}0\)/.test(waveColor) &&
    !(rgb !== null && rgb[1] === rgb[2] && rgb[2] === rgb[3]);
  const previousWaveColor = node.style.getPropertyValue('--antd-wave-shadow-color');
  const hadPreviousWaveColor = node.style.getPropertyValue('--antd-wave-shadow-color') !== '';
  if (meaningful) node.style.setProperty('--antd-wave-shadow-color', waveColor);

  node.removeAttribute('ant-click-animating-without-extra-node');
  void node.offsetWidth;
  node.setAttribute('ant-click-animating-without-extra-node', 'true');
  let cleanupTimer: number | undefined;
  const cleanup = () => {
    node.removeAttribute('ant-click-animating-without-extra-node');
    if (meaningful) {
      if (hadPreviousWaveColor) {
        node.style.setProperty('--antd-wave-shadow-color', previousWaveColor);
      } else {
        node.style.removeProperty('--antd-wave-shadow-color');
      }
    }
    node.removeEventListener('animationend', onEnd);
    if (cleanupTimer !== undefined) window.clearTimeout(cleanupTimer);
  };
  const onEnd = (event: AnimationEvent) => {
    if (event.animationName !== 'fadeEffect') return;
    cleanup();
  };
  node.addEventListener('animationend', onEnd);
  cleanupTimer = window.setTimeout(cleanup, 300);
}
