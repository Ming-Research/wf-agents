// The legend of the flowchart, in the view's line in place of the matrix's while the flowchart is shown: its three kinds
// of line (the main path, the arrows a write runs by their key, the ways back) and the dashed arrow of what people or
// the outside write, the way into a cell, and the matrix's own marks and signs where the flowchart uses them, each named
// by its key.

import { ConditionGlyph, DataGlyphs } from '../Chart';
import { type Mark, NO_MARK, type Spot } from '../matrix';
import { Sign } from '../parts';

export function GraphLegend({ waysBack }: { waysBack: number }) {
  const given = (mark: Partial<Mark>): Spot => ({
    mark: { ...NO_MARK, ...mark },
    line: 'none',
    back: 'none',
    backTo: false,
  });
  return (
    <ul className="legend" aria-label="What the marks mean">
      <li>
        <span className="legend-arrow main" aria-hidden="true" /> main path
      </li>
      <li>
        <span className="legend-arrow" aria-hidden="true" /> triggered_by
      </li>
      <li>
        <span className="legend-arrow back" aria-hidden="true" /> way back ({waysBack})
      </li>
      <li>
        <span className="legend-arrow outside" aria-hidden="true" /> people, outside
      </li>
      <li>
        <span className="legend-node" aria-hidden="true" /> when
      </li>
      <li>
        <ConditionGlyph spot={{ when: false, requires: true }} /> requires
      </li>
      <li>
        <DataGlyphs spot={given({ given: true, kind: 'latest' })} /> context latest
      </li>
      <li>
        <DataGlyphs spot={given({ given: true, kind: 'lastAndLatest' })} /> context last_and_latest
      </li>
      <li>
        <DataGlyphs spot={given({ given: true, kind: 'all' })} /> context all
      </li>
      <li>
        <span className="legend-node lit" aria-hidden="true" /> outputs
      </li>
      <li>
        <Sign name="people" /> people
      </li>
      <li>
        <Sign name="outside" /> outside
      </li>
    </ul>
  );
}
