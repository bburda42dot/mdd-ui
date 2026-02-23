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
Variants -> Variant -> DataDataDictionary Spec -> Static fields
Overview table should only have short name column, no category etc. (only affects strutures, do not change other tables)

Variants -> Variant -> DataDataDictionary Spec -> Static fields -> Static field
The detail view does not need the dop variant but byte size and fixed number of items should be displayed as well. 


## Dynamic Length Fields: 
Variants -> Variant -> DataDataDictionary Spec -> Dynamic Length fields
Overview table should only have short name column, no category etc. (only affects strutures, do not change other tables)

Variants -> Variant -> DataDataDictionary Spec -> Dynamic Length fields  -> Dynamic Length field
The detail view should *not* contain the dop variant type and short name but bit and byte position and the data object prop ref, which should be a link to the DOP and jumping should work.

## End of Pdu fields
Variants -> Variant -> DataDataDictionary Spec -> End of pdus
Overview table should only have short name column, no category etc. (only affects strutures, do not change other tables)

Variants -> Variant -> DataDataDictionary Spec -> End of pdus -> end of pdu
The detail view should show the basic structure ref (linked) + min, max values.

## Mux Dops
Variants -> Variant -> DataDataDictionary Spec -> Mux Dops
Overview table should only have short name column, no category etc. (only affects strutures, do not change other tables)

Variants -> Variant -> DataDataDictionary Spec -> Mux Dop
Information: 
Genernal: 
    Switch key
        DOP -> Link
        Byte Pos 
        Bit Pos
    Default Case: 
        Short name
Cases:
Table:
    Short Name | Struct (links to struct) | Lower Limit | Upper limit

## Unit Spec 
Variants -> Variant -> DataDataDictionary Spec -> Unit Spec
Missing Implement it. 


## Tables
Variants -> Variant -> DataDataDictionary Spec -> Tables
Missing Implement it. 


## Tree view.
The project structure in src/tree should reflect how the tree is shown. All elements that have children should be reflected in the tree as such by using modules.


## Bugs 
* If a tree was expanded after jumping and then backspace is used to go back to the last element, the stored index is wrong and a wrong target is selected. rather store the whole path.
* Not all static fields are listed, some are missing, maybe by not resolving all references. do NOT use parent_ref lookup for this. 
* Using back button on the mouse should work the same as backspace does
* SDGs in the tree should be yellow, not blue
* Review the instructions and fix the code to comply with them.