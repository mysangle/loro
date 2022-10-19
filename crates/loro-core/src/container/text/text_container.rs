use rle::RleTree;
use smallvec::SmallVec;

use crate::{
    container::{list::list_op::ListOp, Container, ContainerID, ContainerType},
    dag::{Dag, DagUtils},
    id::ID,
    log_store::LogStoreRef,
    op::{InsertContent, Op, OpContent, OpProxy},
    span::{HasIdSpan, IdSpan},
    value::LoroValue,
    LogStore,
};

use super::{
    string_pool::StringPool,
    text_content::{ListSlice, ListSliceTreeTrait},
    tracker::Tracker,
};

#[derive(Clone, Debug)]
struct DagNode {
    id: IdSpan,
    deps: SmallVec<[ID; 2]>,
}

#[derive(Debug)]
pub struct TextContainer {
    id: ContainerID,
    log_store: LogStoreRef,
    state: RleTree<ListSlice, ListSliceTreeTrait>,
    raw_str: StringPool,
    tracker: Tracker,
}

impl TextContainer {
    pub fn insert(&mut self, pos: usize, text: &str) -> Option<ID> {
        let id = if let Ok(mut store) = self.log_store.write() {
            let id = store.next_id();
            let slice = ListSlice::from_range(self.raw_str.alloc(text));
            self.state.insert(pos, slice.clone());
            let op = Op::new(
                id,
                OpContent::Normal {
                    content: InsertContent::List(ListOp::Insert { slice, pos }),
                },
                self.id.clone(),
            );
            store.append_local_ops(vec![op]);
            id
        } else {
            unimplemented!()
        };

        Some(id)
    }

    pub fn delete(&mut self, pos: usize, len: usize) -> Option<ID> {
        let id = if let Ok(mut store) = self.log_store.write() {
            let id = store.next_id();
            let op = Op::new(
                id,
                OpContent::Normal {
                    content: InsertContent::List(ListOp::Delete { len, pos }),
                },
                self.id.clone(),
            );

            store.append_local_ops(vec![op]);
            self.state.delete_range(Some(pos), Some(pos + len));
            id
        } else {
            unimplemented!()
        };

        Some(id)
    }
}

impl Container for TextContainer {
    fn id(&self) -> &ContainerID {
        &self.id
    }

    fn type_(&self) -> ContainerType {
        ContainerType::Text
    }

    // TODO: move main logic to tracker module
    fn apply(&mut self, op: &OpProxy, store: &LogStore) {
        let new_op_id = op.id_last();
        let content = op.content_sliced();
        // TODO: may reduce following two into one op
        let common = store.find_common_ancestor(&[new_op_id], store.frontier());
        let path_to_store_head = store.find_path(&common, store.frontier());
        let mut common_vv = store.vv();
        common_vv.retreat(&path_to_store_head.right);

        let mut latest_head: SmallVec<[ID; 2]> = store.frontier().into();
        latest_head.push(new_op_id);
        if common.len() == 0 || !common.iter().all(|x| self.tracker.contains(new_op_id)) {
            // stage 1
            self.tracker = Tracker::new(common.clone(), common_vv);
            let path = store.find_path(&common, &latest_head);
            for iter in store.iter_partial(&common, path.right) {
                self.tracker.retreat(&iter.retreat);
                self.tracker.forward(&iter.forward);
                for op in iter.data.ops.iter() {
                    if op.container == self.id {
                        self.tracker.apply(op.id, &op.content)
                    }
                }
            }
        }

        // stage 2
        let path = store.find_path(&latest_head, &store.frontier());
        self.tracker.retreat(&path.left);
        todo!("turn on tracker and calculate effects")
    }

    fn checkout_version(&mut self, _vv: &crate::VersionVector) {
        todo!()
    }

    fn get_value(&mut self) -> &LoroValue {
        todo!()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
