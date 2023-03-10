pub const SQURRIEL_CODE: &str = r#"
for(int x = 0; x < 16; x++)
{
    entity dummy = CreateExpensiveScriptMoverModel( $"models/humans/heroes/mlt_hero_jack.mdl", <0,0,0>, <0,0,0>, SOLID_VPHYSICS, -1 )
    dummy.kv.skin = PILOT_SKIN_INDEX_GHOST
    dummy.NotSolid()
    dummy.SetScriptName(x.tostring())
}

thread void function() 
{
    bool functionref( entity, vector ) TraceWallrun = bool function( entity player, vector side )
    {
        vector traceStart = player.GetOrigin()
        vector traceEnd = player.GetOrigin() + side
        array<entity> ignoreEnts = [ player ]
    
        TraceResults results = TraceLine( traceStart, traceEnd, ignoreEnts, TRACE_MASK_SHOT, TRACE_COLLISION_GROUP_NONE )
        return IsValid( results.hitEnt )
    }

    for(;;)
    {
        entity player = GetPlayerArray()[0]
        vector origin = player.GetOrigin()
        vector angles = player.GetAngles()
        int action = 2

        if ( player.IsWallRunning() )
        {
            action = 4
            if ( TraceWallrun( player, player.GetRightVector() * 50 ) ) // right
                action = 4
            else if ( TraceWallrun( player, player.GetRightVector() * -50 ) ) // left
                action = 5
            else if ( TraceWallrun( player, player.GetForwardVector() * 50 ) ) // front
                action = 6
            else if ( TraceWallrun( player, player.GetForwardVector() * -50 ) ) // back
                action = 7
        }
        else if ( player.IsCrouched() )
        {
            action = 0
        }
        else if ( !player.IsOnGround() )
        {
            action = 3
        }
        else if ( player.GetVelocity().x > 30 || player.GetVelocity().y > 30 )
            action = 1


        MirrorPlayerRunFrame( origin, angles, action, void function( int index, vector origin, vector angles, int action )
        {
            entity dummy = GetEntByScriptName(index.tostring())
    
            dummy.NonPhysicsMoveTo( origin, 0.1, 0.000000000001, 0.0000000000001 )
            dummy.NonPhysicsRotateTo( angles, 0.1, 0.000000000001, 0.0000000000001 )
            
            string anim; // stand anim

            switch( action )
            {
                case 0: // slide
                    anim = "ACT_MP_CROUCHWALK_FORWARD"
                    break
                case 1: // run
                    anim = "Sprint_mp_forward"
                    break
                case 2: // stand
                    break
                case 3: // jump / fall
                    anim = "jump_start"
                    break
                case 4: // wallrun right
                    anim = "pt_wallrun_hang_right"
                    break
                case 5: // wallrun left
                    anim = "pt_wallrun_hang_left"
                    break
                case 6: // wallrun front
                    anim = "pt_wallrun_hang_front"
                    break
                case 7: // wallrun back
                    anim = "pt_wallrun_hang_up"
                    break
            }

            if ( IsValid( anim ) )
                dummy.Anim_Play( anim )
            else 
                dummy.Anim_Stop()
        } )
        wait 0
    };
}()
"#;