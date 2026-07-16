import useEmblaCarousel, { type UseEmblaCarouselType } from 'embla-carousel-react';
import {
  Children,
  type ComponentProps,
  type KeyboardEvent,
  createContext,
  isValidElement,
  useCallback,
  useContext,
  useEffect,
  useState,
} from 'react';
import { cn } from '@/lib/cn';

export type CarouselApi = UseEmblaCarouselType[1];
type UseCarouselParameters = Parameters<typeof useEmblaCarousel>;
type CarouselOptions = UseCarouselParameters[0];
type CarouselPlugin = UseCarouselParameters[1];
type CarouselOrientation = 'horizontal' | 'vertical';

interface CarouselProps {
  opts?: CarouselOptions;
  plugins?: CarouselPlugin;
  orientation?: CarouselOrientation;
  /** Receives the embla api once ready, e.g. to drive external dot navigation. */
  setApi?: (api: CarouselApi) => void;
}

interface CarouselContextValue {
  activeSlides: readonly number[];
  carouselRef: UseEmblaCarouselType[0];
  api: CarouselApi;
  orientation: CarouselOrientation;
  selectedIndex: number;
}

const CarouselContext = createContext<CarouselContextValue | null>(null);
const CarouselItemPositionContext = createContext<{ index: number; size: number } | null>(null);

export function useCarousel() {
  const context = useContext(CarouselContext);
  if (!context) {
    throw new Error('useCarousel must be used within a <Carousel>');
  }
  return context;
}

/**
 * Embla-backed carousel adapted to the shadcn island house style (data-slot,
 * cn, function components). Provides drag/swipe, orientation-aware keyboard
 * navigation, slide semantics, and focus isolation; consumers can wire their
 * own dot/thumb navigation via `setApi`.
 */
export function Carousel({
  orientation = 'horizontal',
  opts,
  setApi,
  plugins,
  className,
  children,
  onKeyDownCapture,
  tabIndex = 0,
  ...props
}: ComponentProps<'div'> & CarouselProps) {
  const [carouselRef, api] = useEmblaCarousel(
    { ...opts, axis: orientation === 'horizontal' ? 'x' : 'y' },
    plugins,
  );
  const [activeSlides, setActiveSlides] = useState<readonly number[]>([0]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [slideCount, setSlideCount] = useState(0);

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      const previousKey = orientation === 'horizontal' ? 'ArrowLeft' : 'ArrowUp';
      const nextKey = orientation === 'horizontal' ? 'ArrowRight' : 'ArrowDown';
      if (event.key === previousKey) {
        event.preventDefault();
        api?.scrollPrev();
      } else if (event.key === nextKey) {
        event.preventDefault();
        api?.scrollNext();
      }
    },
    [api, orientation],
  );

  useEffect(() => {
    if (!api) return;
    const updateState = () => {
      const nextActiveSlides = api.slidesInView();
      setActiveSlides((current) =>
        current.length === nextActiveSlides.length &&
        current.every((value, index) => value === nextActiveSlides[index])
          ? current
          : nextActiveSlides,
      );
      setSelectedIndex(api.selectedScrollSnap());
      setSlideCount(api.scrollSnapList().length);
    };

    updateState();
    setApi?.(api);
    api.on('select', updateState);
    api.on('reInit', updateState);
    return () => {
      api.off('select', updateState);
      api.off('reInit', updateState);
    };
  }, [api, setApi]);

  return (
    <CarouselContext
      value={{ activeSlides, carouselRef, api, orientation, selectedIndex }}
    >
      <div
        {...props}
        data-slot="carousel"
        role="region"
        aria-roledescription="carousel"
        className={cn('relative', className)}
        tabIndex={tabIndex}
        onKeyDownCapture={(event) => {
          onKeyDownCapture?.(event);
          if (!event.defaultPrevented) handleKeyDown(event);
        }}
      >
        {children}
        {slideCount > 0 ? (
          <span
            data-slot="carousel-status"
            role="status"
            aria-atomic="true"
            aria-live="polite"
            className="sr-only"
          >
            {selectedIndex + 1} / {slideCount}
          </span>
        ) : null}
      </div>
    </CarouselContext>
  );
}

export function CarouselContent({ className, children, ...props }: ComponentProps<'div'>) {
  const { carouselRef, orientation } = useCarousel();
  const items = Children.toArray(children);
  return (
    <div ref={carouselRef} data-slot="carousel-content" className="overflow-hidden">
      <div
        className={cn('flex', orientation === 'horizontal' ? '-ml-4' : '-mt-4 flex-col', className)}
        {...props}
      >
        {items.map((child, index) => (
          <CarouselItemPositionContext
            key={isValidElement(child) && child.key !== null ? child.key : index}
            value={{ index, size: items.length }}
          >
            {child}
          </CarouselItemPositionContext>
        ))}
      </div>
    </div>
  );
}

export function CarouselItem({
  className,
  'aria-current': requestedAriaCurrent,
  'aria-hidden': requestedAriaHidden,
  'aria-label': requestedAriaLabel,
  inert: requestedInert,
  ...props
}: ComponentProps<'div'>) {
  const { activeSlides, orientation, selectedIndex } = useCarousel();
  const position = useContext(CarouselItemPositionContext);
  const index = position?.index ?? 0;
  const active = activeSlides.includes(index);
  const selected = selectedIndex === index;
  return (
    <div
      {...props}
      data-slot="carousel-item"
      role="group"
      aria-roledescription="slide"
      aria-current={selected ? (requestedAriaCurrent ?? true) : undefined}
      aria-hidden={active ? requestedAriaHidden : true}
      aria-label={requestedAriaLabel ?? (position ? `${index + 1} / ${position.size}` : undefined)}
      inert={active ? requestedInert : true}
      className={cn(
        'min-w-0 shrink-0 grow-0 basis-full',
        orientation === 'horizontal' ? 'pl-4' : 'pt-4',
        className,
      )}
    />
  );
}
