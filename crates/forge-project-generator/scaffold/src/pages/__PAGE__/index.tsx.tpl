import Page from './Page';
import { withPageRuntime } from '../../runtime/withPageRuntime';

export default withPageRuntime(Page, { id: '__PAGE_ID__' });
