# Todo


## Structures
Variants -> Variant -> DataDataDictionary Spec -> Structures
Overview table should only have short name column, no category etc. (only affects strutures, do not change other tables)

Variants -> Variant -> Structures -> Structure
The table is not 100% correct. Change it to two tabs.
Overview and Params.
Overview contains high level like is_visile and, param count etc. 
Params is a table that contains all Params
The table should be cell select. Make sure to make the jump target blue.
The columns are 
Short name, byte, bit-len, byte-len, value, dop, semantic.

Jump targets is dop and jumping should be implemented as jumping to the selected dop.


## Static fields
Variants -> Variant -> DataDataDictionary Spec -> Static feilds
Overview table should only have short name column, no category etc. (only affects strutures, do not change other tables)
The detail view does not need the dop variant but byte size and fixed number of items should be displayed as well. 





## Bugs 
* If a tree was expanded after jumping and then backspace is used to go back to the last element, the stored index is wrong and a wrong target is selected. rather store the whole path.
* Not all static fields are listed, some are missing, maybe by not resolving all references. do NOT use parent_ref lookup for this. 
* Using back button on the mouse should work the same as backspace does
