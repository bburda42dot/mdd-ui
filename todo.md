# Todo


## Tables
Variants -> Variant -> DataDataDictionary Spec -> Table
Overview table should only have short name column, no category etc. (only affects strutures, do not change other tables)

Variants -> Variant -> DataDataDictionary Spec -> Tables -> Tables
The detail view should be a table which contains the table rows. 
Table Row | Key | Struct Ref

Structure ref should be clickable


## Jumping
Links are now often just popup, instead the application should not show a popup but navigate to the item in question. 
Links should all be blue.
Example: Variants -> Variant -> DataDataDictionary Spec -> Mux Dops -> Mux Dop: General: Dop shows a popup instead of jumping to the DOP, bascially no popups, except help.


## Functional Classes

Variants -> Variant -> Functional Classes -> Functional Class 
The table contains a lot of duplicated element. THis is because the lookup is finding children and then walking the tree back up again via parent_refs.


## DiagComm
Variants -> Variant ->Diag Comms -> DiagComm

* In Request/Response the first column should jump to the response/response. Currently this does not work. Please note that the jump in DOP is correct and should not be changed.
* Precondition state refs and state transition refs are not looked up correct. It's always empty. You might have to use the state transitions and state charts to do the lookup.


## Tree
* The scroll bar is not updated. Drag to scroll works but scrolling any other way does not update the bar
* When jumping do not expand the whole tree but only what's necessary
* Ecu Shared Data / Functional groups should have the same detail view as variant (top most layer)
* Adjust the search, so it can be scoped to the current view, meaning searching something will hide i.e. all other variants but allow searching for i.e. diagcomm only in this variant. (note variant is only an example should work for all elements.)

## General
* Jumping to a row by typing should always be done by the selected column
* If a table has a short name column. this should always be first 
 
## ComParamRef

Variants -> Variant -> ComParamRef
Detail view not implemented.
Remove type from the tree
Variants -> Variant -> ComParamRef should contain a table with shortname and type
Implement a detail view for a single cp ref similar to existing detail panes.


## Variant

Variants -> Variant
Overview table is indented strange. i.e. "   Diag-Comms" remove this

## Parent Refs
Variant -> Parent Refs
This level should only be a list with available Parent refs. short name and type. L
Parent refs should be children in the tree below parent ref

Variant -> Parent Refs -> Parent Ref
Should be the detail view 
