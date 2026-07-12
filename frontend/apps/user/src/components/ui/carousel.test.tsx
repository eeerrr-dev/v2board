// @vitest-environment jsdom
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { Carousel, CarouselContent, CarouselItem } from './carousel';

const mocks = vi.hoisted(() => ({
  state: {
    count: 1,
    listeners: {} as Record<string, Array<() => void>>,
    selected: 0,
  },
  api: {
    off: vi.fn((event: string, callback: () => void) => {
      mocks.state.listeners[event] = (mocks.state.listeners[event] ?? []).filter(
        (listener) => listener !== callback,
      );
    }),
    on: vi.fn((event: string, callback: () => void) => {
      (mocks.state.listeners[event] ??= []).push(callback);
    }),
    scrollNext: vi.fn(() => {
      mocks.state.selected = Math.min(mocks.state.selected + 1, mocks.state.count - 1);
      (mocks.state.listeners.select ?? []).forEach((callback) => callback());
    }),
    scrollPrev: vi.fn(() => {
      mocks.state.selected = Math.max(mocks.state.selected - 1, 0);
      (mocks.state.listeners.select ?? []).forEach((callback) => callback());
    }),
    scrollSnapList: () => Array.from({ length: mocks.state.count }, (_, index) => index),
    selectedScrollSnap: () => mocks.state.selected,
    slidesInView: () => [mocks.state.selected],
  },
  carouselRef: vi.fn(),
  useEmblaCarousel: vi.fn(),
}));

vi.mock('embla-carousel-react', () => ({
  default: (options: unknown, plugins: unknown) => {
    mocks.useEmblaCarousel(options, plugins);
    return [mocks.carouselRef, mocks.api];
  },
}));

beforeEach(() => {
  mocks.state.count = 1;
  mocks.state.listeners = {};
  mocks.state.selected = 0;
  mocks.carouselRef.mockClear();
  mocks.api.off.mockClear();
  mocks.api.on.mockClear();
  mocks.api.scrollNext.mockClear();
  mocks.api.scrollPrev.mockClear();
  mocks.useEmblaCarousel.mockClear();
});

describe('Carousel', () => {
  it('exposes Embla after commit and delegates keyboard navigation to its API', async () => {
    const setApi = vi.fn();
    render(
      <Carousel setApi={setApi}>
        <CarouselContent>
          <CarouselItem>Slide</CarouselItem>
        </CarouselContent>
      </Carousel>,
    );

    await waitFor(() => expect(setApi).toHaveBeenCalledWith(mocks.api));
    expect(mocks.useEmblaCarousel).toHaveBeenCalledWith({ axis: 'x' }, undefined);

    const region = screen.getByRole('region');
    fireEvent.keyDown(region, { key: 'ArrowLeft' });
    fireEvent.keyDown(region, { key: 'ArrowRight' });

    expect(mocks.api.scrollPrev).toHaveBeenCalledTimes(1);
    expect(mocks.api.scrollNext).toHaveBeenCalledTimes(1);
  });

  it('isolates inactive slides and announces the selected position', async () => {
    mocks.state.count = 2;
    render(
      <Carousel aria-label="Notices">
        <CarouselContent>
          <CarouselItem>
            <button type="button">First</button>
          </CarouselItem>
          <CarouselItem>
            <button type="button">Second</button>
          </CarouselItem>
        </CarouselContent>
      </Carousel>,
    );

    const slides = screen.getAllByRole('group', { hidden: true });
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('1 / 2'));
    expect(slides[0]).toHaveAttribute('aria-current', 'true');
    expect(slides[0]).toHaveAttribute('aria-label', '1 / 2');
    expect(slides[0]).not.toHaveAttribute('aria-posinset');
    expect(slides[0]).not.toHaveAttribute('aria-setsize');
    expect(slides[0]).not.toHaveAttribute('inert');
    expect(slides[1]).toHaveAttribute('aria-hidden', 'true');
    expect(slides[1]).toHaveAttribute('inert');

    fireEvent.keyDown(screen.getByRole('region'), { key: 'ArrowRight' });

    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('2 / 2'));
    expect(slides[0]).toHaveAttribute('aria-hidden', 'true');
    expect(slides[0]).toHaveAttribute('inert');
    expect(slides[1]).toHaveAttribute('aria-current', 'true');
    expect(slides[1]).not.toHaveAttribute('inert');
  });

  it('uses up and down arrows for a vertical carousel', () => {
    render(
      <Carousel orientation="vertical">
        <CarouselContent>
          <CarouselItem>Slide</CarouselItem>
        </CarouselContent>
      </Carousel>,
    );

    const region = screen.getByRole('region');
    fireEvent.keyDown(region, { key: 'ArrowUp' });
    fireEvent.keyDown(region, { key: 'ArrowDown' });
    fireEvent.keyDown(region, { key: 'ArrowRight' });

    expect(mocks.api.scrollPrev).toHaveBeenCalledTimes(1);
    expect(mocks.api.scrollNext).toHaveBeenCalledTimes(1);
  });
});
